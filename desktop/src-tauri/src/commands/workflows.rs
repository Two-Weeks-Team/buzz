use std::collections::HashSet;

use serde::Serialize;
use serde_json::Value;
use tauri::State;

use crate::{
    app_state::AppState,
    events,
    relay::{get_relay_json, parse_command_response, query_relay, submit_event},
};

// ── Wire shapes (snake_case, consumed by tauriWorkflows.ts) ──────────────────

/// A workflow definition as the desktop frontend expects it. Mirrors the
/// `RawWorkflow` type in `desktop/src/shared/api/tauriWorkflows.ts`.
///
/// The relay stores a workflow as a single kind:30620 event whose content is
/// the raw YAML. Everything the UI needs is derived from that event:
/// - `id` / `channel_id` from the `d` / `h` tags,
/// - `definition` from parsing the YAML body into a free-form object,
/// - `name` from `definition.name`,
/// - `owner_pubkey` / timestamps from the event itself.
///
/// `status` is always `"active"` here: the relay's disable/archive lifecycle is
/// not reflected back into the kind:30620 event, and the UI derives a
/// "disabled" display state from `definition.enabled` on its own
/// (`getWorkflowDisplayStatus`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WorkflowWire {
    pub id: String,
    /// Event id of the current kind:30620 revision, used for conflict-protected updates.
    pub revision: String,
    pub name: String,
    pub owner_pubkey: String,
    pub channel_id: Option<String>,
    pub definition: Value,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Response shape for create/update. Mirrors `RawWorkflowSaveResponse` in the
/// frontend: a full workflow record plus an optional webhook secret (only
/// present for webhook-triggered workflows on creation).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WorkflowSaveWire {
    #[serde(flatten)]
    pub workflow: WorkflowWire,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_secret: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, Serialize, PartialEq)]
pub struct WorkflowRunCursorWire {
    pub before: String,
    pub before_id: String,
}

#[derive(Debug, Clone, serde::Deserialize, Serialize, PartialEq)]
pub struct WorkflowRunsWire {
    pub runs: Vec<Value>,
    pub next: Option<WorkflowRunCursorWire>,
}

#[derive(Debug, Clone, serde::Deserialize, Serialize, PartialEq)]
pub struct WorkflowApprovalsWire {
    pub approvals: Vec<Value>,
}

/// Canonical trigger acknowledgement consumed by the Desktop client.
///
/// The relay currently returns only `run_id`; the workflow id is the command
/// input and a newly-created run always begins pending. Keeping that adaptation
/// here prevents the frontend from guessing fields or confusing the trigger
/// event id with the persisted run id.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WorkflowTriggerWire {
    pub run_id: String,
    pub workflow_id: String,
    pub status: String,
}

#[derive(Debug, serde::Deserialize)]
struct WorkflowTriggerAck {
    run_id: String,
}

// ── Reads ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_channel_workflows(
    channel_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<WorkflowWire>, String> {
    let events = query_relay(
        &state,
        &[serde_json::json!({
            "kinds": [30620],
            "#h": [channel_id],
        })],
    )
    .await?;

    Ok(events.iter().map(workflow_from_event).collect())
}

// Keep this aligned with the relay's aggregate explicit-`#h` request bound.
// Each filter below carries exactly one explicit value so old relays retain the
// known-compatible shape while current relays cannot reject large memberships.
const WORKFLOW_QUERY_CHANNEL_BATCH_SIZE: usize = 128;

/// Fetch workflows across many channels using bounded relay round-trips.
///
/// The Workflows overview screen previously issued one `get_channel_workflows`
/// query per member channel (`Promise.all` fanout in `WorkflowsView`), i.e. N
/// relay POSTs. This sends one single-channel filter per channel, in requests of
/// at most 128 filters. Using one multi-value `#h` filter is equivalent under
/// NIP-01, but older relays incorrectly narrowed that shape to its first
/// channel. Each `WorkflowWire` carries its own `channel_id` (from the event's
/// `h` tag), so the frontend can still group results by channel. Neither this
/// nor the per-channel command sets a `limit`, so batching does not change
/// result completeness. Results are deduplicated by signed event ID in case a
/// caller supplies duplicate channel IDs.
#[tauri::command]
pub async fn get_channels_workflows(
    channel_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<WorkflowWire>, String> {
    let filter_batches = channel_workflow_filter_batches(channel_ids)?;
    let mut seen_event_ids = HashSet::new();
    let mut workflows = Vec::new();

    for filters in filter_batches {
        let events = query_relay(&state, &filters).await?;
        append_unique_workflows(&mut workflows, &mut seen_event_ids, &events);
    }

    Ok(workflows)
}

fn append_unique_workflows(
    workflows: &mut Vec<WorkflowWire>,
    seen_event_ids: &mut HashSet<nostr::EventId>,
    events: &[nostr::Event],
) {
    workflows.extend(
        events
            .iter()
            .filter(|event| seen_event_ids.insert(event.id))
            .map(workflow_from_event),
    );
}

fn channel_workflow_filter_batches(channel_ids: Vec<String>) -> Result<Vec<Vec<Value>>, String> {
    let filters = channel_workflow_filters(channel_ids)?;
    Ok(filters
        .chunks(WORKFLOW_QUERY_CHANNEL_BATCH_SIZE)
        .map(<[Value]>::to_vec)
        .collect())
}

fn channel_workflow_filters(channel_ids: Vec<String>) -> Result<Vec<Value>, String> {
    channel_ids
        .into_iter()
        .map(|channel_id| {
            let channel_id = uuid::Uuid::parse_str(channel_id.trim())
                .map_err(|_| "invalid channel id".to_string())?;
            Ok(serde_json::json!({
                "kinds": [30620],
                "#h": [channel_id.to_string()],
            }))
        })
        .collect()
}

#[tauri::command]
pub async fn get_workflow(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<WorkflowWire, String> {
    let events = query_relay(
        &state,
        &[serde_json::json!({
            "kinds": [30620],
            "#d": [workflow_id],
            "limit": 1
        })],
    )
    .await?;

    events
        .first()
        .map(workflow_from_event)
        .ok_or_else(|| "workflow not found".to_string())
}

#[tauri::command]
pub async fn get_workflow_runs(
    workflow_id: String,
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> Result<WorkflowRunsWire, String> {
    let workflow_id =
        uuid::Uuid::parse_str(&workflow_id).map_err(|_| "invalid workflow id".to_string())?;
    let limit = limit.unwrap_or(20).clamp(1, 100);
    get_relay_json(
        &state,
        &format!("/workflows/{workflow_id}/runs?limit={limit}"),
    )
    .await
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowAttemptsRequest {
    workflow_id: String,
    run_id: String,
    after_index: Option<u32>,
    expected_relay_url: String,
    expected_signer_pubkey: String,
}

fn workflow_attempts_path(
    request: &WorkflowAttemptsRequest,
) -> Result<(String, uuid::Uuid, uuid::Uuid), String> {
    let workflow =
        uuid::Uuid::parse_str(&request.workflow_id).map_err(|_| "invalid workflow id")?;
    let run = uuid::Uuid::parse_str(&request.run_id).map_err(|_| "invalid workflow run id")?;
    if request.after_index.is_some_and(|index| index >= 4096) {
        return Err("invalid journal cursor".into());
    }
    let mut path = format!("/workflows/{workflow}/runs/{run}/attempts?limit=16");
    if let Some(index) = request.after_index {
        path.push_str(&format!("&after_index={index}"));
    }
    Ok((path, workflow, run))
}

/// Read one journal page with a captured relay/signer; never retarget on switch.
#[tauri::command]
pub async fn get_run_attempts(
    request: WorkflowAttemptsRequest,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    let (path, workflow, run) = workflow_attempts_path(&request)?;
    if request.expected_relay_url.trim().is_empty() || request.expected_signer_pubkey.is_empty() {
        return Err("journal read requires relay and signer scope".into());
    }
    let relay_base = crate::relay::relay_api_base_url_with_override(&state);
    let keys = state.signing_keys()?;
    crate::relay::assert_expected_relay_scope(Some(&request.expected_relay_url), &relay_base)?;
    crate::relay::assert_expected_signer(
        Some(&request.expected_signer_pubkey),
        &keys.public_key().to_hex(),
    )?;
    let response: Value =
        crate::relay::get_relay_json_at_with_keys(&state, &path, &relay_base, &keys).await?;
    if response["workflow_id"].as_str() != Some(workflow.to_string().as_str())
        || response["run_id"].as_str() != Some(run.to_string().as_str())
    {
        return Err("journal response scope mismatch".into());
    }
    Ok(response)
}

// ── Writes ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn create_workflow(
    channel_id: String,
    yaml_definition: String,
    state: State<'_, AppState>,
) -> Result<WorkflowSaveWire, String> {
    let workflow_id = uuid::Uuid::new_v4().to_string();
    let builder =
        events::build_workflow_definition(&workflow_id, &channel_id, &yaml_definition, None)?;
    let result = submit_event(builder, &state).await?;

    // The relay returns `webhook_secret` in the OK response message for
    // webhook-triggered workflows. Everything else in the save record is built
    // locally from the inputs we already hold — the relay's create response
    // only carries `{ workflow_id, webhook_secret? }`.
    let webhook_secret = parse_command_response::<Value>(&result.message)
        .ok()
        .and_then(|v| {
            v.get("webhook_secret")
                .and_then(Value::as_str)
                .map(str::to_string)
        });

    let now = now_secs();
    let workflow = workflow_record(
        workflow_id,
        result.event_id,
        Some(channel_id),
        current_pubkey_hex(&state)?,
        &yaml_definition,
        now,
        now,
    );

    Ok(WorkflowSaveWire {
        workflow,
        webhook_secret,
    })
}

#[tauri::command]
pub async fn update_workflow(
    workflow_id: String,
    yaml_definition: String,
    expected_revision: String,
    state: State<'_, AppState>,
) -> Result<WorkflowSaveWire, String> {
    // Find the channel id (and creation time) from the existing workflow event
    // so the new event carries the same `h` tag — kind:30620 is replaceable by
    // (pubkey, d-tag).
    let prior = query_relay(
        &state,
        &[serde_json::json!({
            "kinds": [30620],
            "#d": [workflow_id.clone()],
            "limit": 1
        })],
    )
    .await?;

    let prior_event = prior
        .first()
        .ok_or_else(|| "workflow not found".to_string())?;
    if prior_event.id.to_hex() != expected_revision {
        return Err("workflow changed since it was loaded; refresh and try again".to_string());
    }
    let channel_id = tag_value(prior_event, "h").ok_or_else(|| "workflow not found".to_string())?;
    let created_at = prior_event.created_at.as_secs() as i64;

    let builder = events::build_workflow_definition(
        &workflow_id,
        &channel_id,
        &yaml_definition,
        Some(&expected_revision),
    )?;
    let result = submit_event(builder, &state).await?;

    let updated_at = now_secs();
    let workflow = workflow_record(
        workflow_id,
        result.event_id,
        Some(channel_id),
        current_pubkey_hex(&state)?,
        &yaml_definition,
        created_at,
        updated_at,
    );

    Ok(WorkflowSaveWire {
        workflow,
        // Updates never rotate the webhook secret.
        webhook_secret: None,
    })
}

#[tauri::command]
pub async fn delete_workflow(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let builder = events::build_workflow_delete(&workflow_id, &current_pubkey_hex(&state)?)?;
    submit_event(builder, &state).await?;
    Ok(())
}

#[tauri::command]
pub async fn trigger_workflow(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<WorkflowTriggerWire, String> {
    let builder = events::build_workflow_trigger(&workflow_id)?;
    let result = submit_event(builder, &state).await?;
    trigger_wire_from_message(workflow_id, &result.message)
}

// ── Approvals ────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowApprovalDecisionRequest {
    workflow_id: String,
    run_id: String,
    approval_ref: String,
    expected_relay_url: String,
    expected_signer_pubkey: String,
    note: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct WorkflowApprovalDecisionWire {
    approval_ref: String,
    workflow_id: String,
    run_id: String,
    status: String,
    event_id: String,
}

#[derive(serde::Deserialize)]
struct WorkflowApprovalDecisionAck {
    run_id: String,
    status: String,
}

#[derive(serde::Deserialize)]
struct PendingApprovalBinding {
    approval_ref: String,
    workflow_id: String,
    run_id: String,
    status: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(serde::Deserialize)]
struct PendingApprovalBindings {
    approvals: Vec<PendingApprovalBinding>,
}

fn approval_binding_matches(
    approval: &PendingApprovalBinding,
    request: &WorkflowApprovalDecisionRequest,
    now: chrono::DateTime<chrono::Utc>,
) -> bool {
    approval.approval_ref == request.approval_ref
        && approval.workflow_id == request.workflow_id
        && approval.run_id == request.run_id
        && approval.status == "pending"
        && approval.expires_at > now
}

fn approval_wire_from_message(
    request: &WorkflowApprovalDecisionRequest,
    expected_status: &str,
    event_id: String,
    message: &str,
) -> Result<WorkflowApprovalDecisionWire, String> {
    let ack: WorkflowApprovalDecisionAck = parse_command_response(message)?;
    if ack.run_id != request.run_id || ack.status != expected_status {
        return Err("approval decision response did not match the requested run/action; refresh history before any retry".to_string());
    }
    Ok(WorkflowApprovalDecisionWire {
        approval_ref: request.approval_ref.clone(),
        workflow_id: request.workflow_id.clone(),
        run_id: ack.run_id,
        status: ack.status,
        event_id,
    })
}

async fn submit_approval_decision(
    mut request: WorkflowApprovalDecisionRequest,
    granted: bool,
    state: &AppState,
) -> Result<WorkflowApprovalDecisionWire, String> {
    if request.expected_relay_url.trim().is_empty() {
        return Err("approval decision requires a captured relay scope".to_string());
    }
    nostr::PublicKey::from_hex(&request.expected_signer_pubkey)
        .map_err(|_| "approval decision requires a captured signer public key".to_string())?;
    request.workflow_id = uuid::Uuid::parse_str(&request.workflow_id)
        .map_err(|_| "invalid workflow id".to_string())?
        .to_string();
    request.run_id = uuid::Uuid::parse_str(&request.run_id)
        .map_err(|_| "invalid workflow run id".to_string())?
        .to_string();
    request.approval_ref = request.approval_ref.to_ascii_lowercase();
    let builder = if granted {
        events::build_approval_grant(&request.approval_ref, request.note.as_deref())?
    } else {
        events::build_approval_deny(&request.approval_ref, request.note.as_deref())?
    };
    // Capture and assert both parts before any await. A community switch may
    // not retarget the preflight read, signed command, or NIP-98 authentication.
    let relay_base = crate::relay::relay_api_base_url_with_override(state);
    let keys = state.signing_keys()?;
    crate::relay::assert_expected_relay_scope(Some(&request.expected_relay_url), &relay_base)?;
    crate::relay::assert_expected_signer(
        Some(&request.expected_signer_pubkey),
        &keys.public_key().to_hex(),
    )?;
    let approvals: PendingApprovalBindings = crate::relay::get_relay_json_at_with_keys(
        state,
        &format!(
            "/workflows/{}/runs/{}/approvals",
            request.workflow_id, request.run_id
        ),
        &relay_base,
        &keys,
    )
    .await?;
    if approvals
        .approvals
        .iter()
        .filter(|approval| approval_binding_matches(approval, &request, chrono::Utc::now()))
        .count()
        != 1
    {
        return Err(
            "exact pending approval is unavailable or expired; refresh history".to_string(),
        );
    }
    // The relay remains authoritative for designated approver, write-time
    // expiry and duplicate/racing decisions. Never automatically retry a write.
    let result =
        crate::relay::submit_event_at_with_keys(builder, state, &relay_base, &keys).await?;
    approval_wire_from_message(
        &request,
        if granted { "granted" } else { "denied" },
        result.event_id,
        &result.message,
    )
}

#[tauri::command]
pub async fn get_run_approvals(
    workflow_id: String,
    run_id: String,
    state: State<'_, AppState>,
) -> Result<WorkflowApprovalsWire, String> {
    let workflow_id =
        uuid::Uuid::parse_str(&workflow_id).map_err(|_| "invalid workflow id".to_string())?;
    let run_id =
        uuid::Uuid::parse_str(&run_id).map_err(|_| "invalid workflow run id".to_string())?;
    get_relay_json(
        &state,
        &format!("/workflows/{workflow_id}/runs/{run_id}/approvals"),
    )
    .await
}

#[tauri::command]
pub async fn grant_approval(
    request: WorkflowApprovalDecisionRequest,
    state: State<'_, AppState>,
) -> Result<WorkflowApprovalDecisionWire, String> {
    submit_approval_decision(request, true, &state).await
}

#[tauri::command]
pub async fn deny_approval(
    request: WorkflowApprovalDecisionRequest,
    state: State<'_, AppState>,
) -> Result<WorkflowApprovalDecisionWire, String> {
    submit_approval_decision(request, false, &state).await
}

// ── Helpers (pure, unit-tested in workflows_tests.rs) ─────────────────────────

fn trigger_wire_from_message(
    workflow_id: String,
    message: &str,
) -> Result<WorkflowTriggerWire, String> {
    let ack: WorkflowTriggerAck = parse_command_response(message)?;
    if ack.run_id.trim().is_empty() {
        return Err("workflow trigger response contained an empty run_id".to_string());
    }
    Ok(WorkflowTriggerWire {
        run_id: ack.run_id,
        workflow_id,
        status: "pending".to_string(),
    })
}

fn current_pubkey_hex(state: &AppState) -> Result<String, String> {
    let keys = state.keys.lock().map_err(|e| e.to_string())?;
    Ok(keys.public_key().to_hex())
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

/// First value of the tag whose name matches `name` (e.g. `d`, `h`).
fn tag_value(ev: &nostr::Event, name: &str) -> Option<String> {
    ev.tags.iter().find_map(|t| {
        let s = t.as_slice();
        (s.len() >= 2 && s[0] == name).then(|| s[1].clone())
    })
}

/// Parse a workflow's YAML body into a free-form JSON object. The frontend
/// consumes `definition` as `Record<string, unknown>`, so we preserve the full
/// document. On parse failure (or a non-object document) we fall back to an
/// empty object rather than failing the whole list query — a single malformed
/// workflow must not break the page.
fn parse_definition(yaml: &str) -> Value {
    match serde_yaml::from_str::<Value>(yaml) {
        Ok(v @ Value::Object(_)) => v,
        _ => Value::Object(serde_json::Map::new()),
    }
}

/// Build a [`WorkflowWire`] record from its parts. Shared by the read path
/// (from a relay event) and the write path (from local inputs).
fn workflow_record(
    id: String,
    revision: String,
    channel_id: Option<String>,
    owner_pubkey: String,
    yaml_definition: &str,
    created_at: i64,
    updated_at: i64,
) -> WorkflowWire {
    let definition = parse_definition(yaml_definition);
    let name = definition
        .get("name")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| id.clone());

    WorkflowWire {
        id,
        revision,
        name,
        owner_pubkey,
        channel_id,
        definition,
        status: "active".to_string(),
        created_at,
        updated_at,
    }
}

/// Convert a kind:30620 workflow definition event into a [`WorkflowWire`].
fn workflow_from_event(ev: &nostr::Event) -> WorkflowWire {
    let id = tag_value(ev, "d").unwrap_or_default();
    let channel_id = tag_value(ev, "h");
    let ts = ev.created_at.as_secs() as i64;
    workflow_record(
        id,
        ev.id.to_hex(),
        channel_id,
        ev.pubkey.to_hex(),
        &ev.content,
        ts,
        ts,
    )
}

#[cfg(test)]
#[path = "workflows_tests.rs"]
mod tests;
