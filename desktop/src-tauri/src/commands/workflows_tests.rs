// Tests for commands/workflows.rs — split into a sibling file to keep
// workflows.rs focused. These exercise the pure helpers (no relay): event →
// wire conversion, YAML definition parsing, name derivation, and the
// create/update record shaping.

use super::*;
use nostr::{EventBuilder, Keys, Kind, Tag};

/// Build a signed kind:30620 workflow definition event with the given YAML
/// content and d/h tags.
fn wf_event(d: &str, h: &str, yaml: &str) -> nostr::Event {
    let keys = Keys::generate();
    let tags: Vec<Tag> = [vec!["d", d], vec!["h", h]]
        .into_iter()
        .map(|t| Tag::parse(t).expect("parse tag"))
        .collect();
    EventBuilder::new(Kind::Custom(30620), yaml)
        .tags(tags)
        .sign_with_keys(&keys)
        .expect("sign")
}

const CHAN: &str = "11111111-1111-1111-1111-111111111111";
const WF: &str = "22222222-2222-2222-2222-222222222222";

fn approval_request() -> WorkflowApprovalDecisionRequest {
    WorkflowApprovalDecisionRequest {
        workflow_id: WF.to_string(),
        run_id: CHAN.to_string(),
        approval_ref: "ab".repeat(32),
        expected_relay_url: "ws://127.0.0.1:63203".to_string(),
        expected_signer_pubkey: "cd".repeat(32),
        note: Some("scope only".to_string()),
    }
}

/// Explicit opt-in only. The fixture contains run IDs/references, never keys;
/// the only signing identities are fixed synthetic loopback-lab keys 1 and 3.
#[tokio::test]
#[ignore = "requires an explicitly provisioned synthetic relay on 127.0.0.1:63203"]
async fn desktop_approval_loopback_probe() {
    #[derive(serde::Deserialize)]
    struct Fixture {
        workflow_id: String,
        run_id: String,
        approval_ref: String,
        granted: bool,
    }
    let path =
        std::env::var("BUZZ_DESKTOP_APPROVAL_FIXTURE").expect("explicit fixture file required");
    let fixture: Fixture = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let owner = Keys::parse(&format!("{:064x}", 1)).unwrap();
    let member = Keys::parse(&format!("{:064x}", 3)).unwrap();
    let state = crate::app_state::build_app_state();
    *state.relay_url_override.lock().unwrap() = Some("ws://127.0.0.1:63203".into());
    *state.keys.lock().unwrap() = owner.clone();
    let request = || WorkflowApprovalDecisionRequest {
        workflow_id: fixture.workflow_id.clone(),
        run_id: fixture.run_id.clone(),
        approval_ref: fixture.approval_ref.clone(),
        expected_relay_url: "ws://127.0.0.1:63203".into(),
        expected_signer_pubkey: owner.public_key().to_hex(),
        note: Some("synthetic desktop decision".into()),
    };
    let mut wrong = request();
    wrong.expected_relay_url = "ws://127.0.0.1:63209".into();
    assert!(submit_approval_decision(wrong, fixture.granted, &state)
        .await
        .is_err());
    let mut wrong = request();
    wrong.expected_signer_pubkey = member.public_key().to_hex();
    assert!(submit_approval_decision(wrong, fixture.granted, &state)
        .await
        .is_err());
    let mut wrong = request();
    wrong.approval_ref = "00".repeat(32);
    assert!(submit_approval_decision(wrong, fixture.granted, &state)
        .await
        .is_err());
    let mut wrong = request();
    wrong.run_id = uuid::Uuid::new_v4().to_string();
    assert!(submit_approval_decision(wrong, fixture.granted, &state)
        .await
        .is_err());
    *state.keys.lock().unwrap() = member.clone();
    let mut wrong = request();
    wrong.expected_signer_pubkey = member.public_key().to_hex();
    assert!(submit_approval_decision(wrong, fixture.granted, &state)
        .await
        .is_err());
    *state.keys.lock().unwrap() = owner.clone();
    let result = submit_approval_decision(request(), fixture.granted, &state)
        .await
        .unwrap();
    assert_eq!(
        result.status,
        if fixture.granted { "granted" } else { "denied" }
    );
    assert_eq!(result.run_id, fixture.run_id);
    assert!(submit_approval_decision(request(), fixture.granted, &state)
        .await
        .is_err());
    use sha2::Digest;
    let executable = std::fs::read(std::env::current_exe().unwrap()).unwrap();
    let mut evidence = serde_json::to_value(&result).unwrap();
    evidence["test_executable_sha256"] = hex::encode(sha2::Sha256::digest(executable)).into();
    std::fs::write(
        format!("{path}.result.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
}

#[test]
fn approval_decision_requires_exact_pending_binding() {
    let request = approval_request();
    let now = chrono::Utc::now();
    let mut approval = PendingApprovalBinding {
        approval_ref: request.approval_ref.clone(),
        workflow_id: request.workflow_id.clone(),
        run_id: request.run_id.clone(),
        status: "pending".to_string(),
        expires_at: now + chrono::Duration::seconds(60),
    };
    assert!(approval_binding_matches(&approval, &request, now));
    for field in ["approvalRef", "workflowId", "runId"] {
        let mut foreign = approval_request();
        match field {
            "approvalRef" => foreign.approval_ref = "00".repeat(32),
            "workflowId" => foreign.workflow_id = CHAN.to_string(),
            _ => foreign.run_id = WF.to_string(),
        }
        assert!(!approval_binding_matches(&approval, &foreign, now));
    }
    for status in ["granted", "denied", "expired", "unknown"] {
        approval.status = status.to_string();
        assert!(!approval_binding_matches(&approval, &request, now));
    }
    approval.status = "pending".to_string();
    approval.expires_at = now;
    assert!(!approval_binding_matches(&approval, &request, now));
}

#[test]
fn approval_decision_ack_is_not_event_acceptance_or_run_completion() {
    let request = approval_request();
    for status in ["granted", "denied"] {
        let message = format!(
            "response:{}",
            serde_json::json!({"run_id": CHAN, "status": status})
        );
        let result =
            approval_wire_from_message(&request, status, "event-id".into(), &message).unwrap();
        let json = serde_json::to_value(result).unwrap();
        assert_eq!(json["approval_ref"], request.approval_ref);
        assert_eq!(json["workflow_id"], WF);
        assert_eq!(json["run_id"], CHAN);
        assert_eq!(json["status"], status);
        assert_eq!(json["event_id"], "event-id");
        assert!(json.get("token").is_none());
    }
    for invalid in [
        "duplicate: already processed".to_string(),
        "response:{}".to_string(),
        "response:{\"event_id\":\"accepted-only\"}".to_string(),
        format!(
            "response:{}",
            serde_json::json!({"run_id": WF, "status":"granted"})
        ),
        format!(
            "response:{}",
            serde_json::json!({"run_id": CHAN, "status":"denied"})
        ),
        format!(
            "response:{}",
            serde_json::json!({"run_id": CHAN, "status":"completed"})
        ),
    ] {
        assert!(
            approval_wire_from_message(&request, "granted", "event-id".into(), &invalid).is_err()
        );
    }
}

const YAML: &str = "\
name: Greet on join
description: Says hi
enabled: true
trigger:
  on: message_posted
  filter: hello
steps:
  - id: reply
    action: post_message
";

#[test]
fn workflow_from_event_maps_all_fields() {
    let ev = wf_event(WF, CHAN, YAML);
    let wf = workflow_from_event(&ev);

    assert_eq!(wf.id, WF);
    assert_eq!(wf.revision, ev.id.to_hex());
    assert_eq!(wf.channel_id.as_deref(), Some(CHAN));
    assert_eq!(wf.owner_pubkey, ev.pubkey.to_hex());
    assert_eq!(wf.name, "Greet on join");
    assert_eq!(wf.status, "active");
    assert_eq!(wf.created_at, ev.created_at.as_secs() as i64);
    assert_eq!(wf.updated_at, ev.created_at.as_secs() as i64);
}

#[test]
fn definition_is_parsed_into_object_with_nested_fields() {
    let ev = wf_event(WF, CHAN, YAML);
    let wf = workflow_from_event(&ev);

    // The whole YAML document is preserved as a free-form object.
    let def = wf.definition.as_object().expect("definition is an object");
    assert_eq!(
        def.get("description").and_then(Value::as_str),
        Some("Says hi")
    );
    assert_eq!(def.get("enabled").and_then(Value::as_bool), Some(true));
    assert_eq!(
        wf.definition.pointer("/trigger/on").and_then(Value::as_str),
        Some("message_posted")
    );
    assert_eq!(
        wf.definition
            .pointer("/steps/0/action")
            .and_then(Value::as_str),
        Some("post_message")
    );
}

#[test]
fn name_falls_back_to_id_when_missing() {
    let yaml = "trigger:\n  on: schedule\n  cron: '* * * * *'\n";
    let ev = wf_event(WF, CHAN, yaml);
    let wf = workflow_from_event(&ev);
    assert_eq!(wf.name, WF);
}

#[test]
fn name_falls_back_to_id_when_blank() {
    let yaml = "name: '   '\ntrigger:\n  on: schedule\n";
    let ev = wf_event(WF, CHAN, yaml);
    let wf = workflow_from_event(&ev);
    assert_eq!(wf.name, WF);
}

#[test]
fn malformed_yaml_yields_empty_object_not_error() {
    // A broken workflow must not break the whole list — definition falls back
    // to an empty object and the name falls back to the id. (YAML is permissive,
    // so this uses an unterminated flow mapping that genuinely fails to parse.)
    let ev = wf_event(WF, CHAN, "{ name: oops, unterminated: [1, 2");
    let wf = workflow_from_event(&ev);
    assert_eq!(wf.definition, Value::Object(serde_json::Map::new()));
    assert_eq!(wf.name, WF);
}

#[test]
fn scalar_yaml_document_yields_empty_object() {
    // A bare scalar parses as valid YAML but isn't an object; treat as empty.
    let ev = wf_event(WF, CHAN, "just a string");
    let wf = workflow_from_event(&ev);
    assert_eq!(wf.definition, Value::Object(serde_json::Map::new()));
}

#[test]
fn tag_value_reads_d_and_h_and_misses_absent() {
    let ev = wf_event(WF, CHAN, YAML);
    assert_eq!(tag_value(&ev, "d").as_deref(), Some(WF));
    assert_eq!(tag_value(&ev, "h").as_deref(), Some(CHAN));
    assert_eq!(tag_value(&ev, "z"), None);
}

#[test]
fn workflow_record_shapes_save_inputs() {
    let wf = workflow_record(
        WF.to_string(),
        "revision-1".to_string(),
        Some(CHAN.to_string()),
        "deadbeef".to_string(),
        YAML,
        100,
        200,
    );
    assert_eq!(wf.id, WF);
    assert_eq!(wf.name, "Greet on join");
    assert_eq!(wf.owner_pubkey, "deadbeef");
    assert_eq!(wf.channel_id.as_deref(), Some(CHAN));
    assert_eq!(wf.created_at, 100);
    assert_eq!(wf.updated_at, 200);
    assert_eq!(wf.status, "active");
}

#[test]
fn save_wire_serializes_flat_with_optional_secret() {
    let workflow = workflow_record(
        WF.to_string(),
        "revision-1".to_string(),
        Some(CHAN.to_string()),
        "deadbeef".to_string(),
        YAML,
        1,
        1,
    );

    // With a secret: present, flattened alongside the workflow fields.
    let with = WorkflowSaveWire {
        workflow: workflow.clone(),
        webhook_secret: Some("s3cr3t".to_string()),
    };
    let v = serde_json::to_value(&with).expect("serialize");
    assert_eq!(v.get("id").and_then(Value::as_str), Some(WF));
    assert_eq!(v.get("name").and_then(Value::as_str), Some("Greet on join"));
    assert_eq!(
        v.get("webhook_secret").and_then(Value::as_str),
        Some("s3cr3t")
    );

    // Without a secret: the key is omitted entirely (frontend treats as null).
    let without = WorkflowSaveWire {
        workflow,
        webhook_secret: None,
    };
    let v = serde_json::to_value(&without).expect("serialize");
    assert!(v.get("webhook_secret").is_none());
    assert_eq!(v.get("id").and_then(Value::as_str), Some(WF));
}

#[test]
fn workflow_wire_serializes_with_snake_case_keys() {
    // Guard the wire contract the frontend's RawWorkflow depends on.
    let ev = wf_event(WF, CHAN, YAML);
    let v = serde_json::to_value(workflow_from_event(&ev)).expect("serialize");
    for key in [
        "id",
        "revision",
        "name",
        "owner_pubkey",
        "channel_id",
        "definition",
        "status",
        "created_at",
        "updated_at",
    ] {
        assert!(v.get(key).is_some(), "missing wire key: {key}");
    }
}

#[test]
fn multi_channel_workflow_query_uses_one_filter_per_channel() {
    let other_channel = "33333333-3333-3333-3333-333333333333";
    let filters = channel_workflow_filters(vec![CHAN.to_string(), other_channel.to_string()])
        .expect("valid channels");

    assert_eq!(filters.len(), 2);
    assert_eq!(
        filters[0],
        serde_json::json!({
            "kinds": [30620],
            "#h": [CHAN],
        })
    );
    assert_eq!(
        filters[1],
        serde_json::json!({
            "kinds": [30620],
            "#h": [other_channel],
        })
    );
}

#[test]
fn workflow_queries_respect_relay_explicit_channel_limit() {
    for (channel_count, expected_batch_sizes) in [
        (WORKFLOW_QUERY_CHANNEL_BATCH_SIZE, vec![128]),
        (WORKFLOW_QUERY_CHANNEL_BATCH_SIZE + 1, vec![128, 1]),
    ] {
        let channel_ids = (0..channel_count)
            .map(|index| uuid::Uuid::from_u128(index as u128 + 1).to_string())
            .collect();
        let batches = channel_workflow_filter_batches(channel_ids).expect("valid channels");

        assert_eq!(
            batches.iter().map(Vec::len).collect::<Vec<_>>(),
            expected_batch_sizes
        );
        assert!(batches.iter().flatten().all(|filter| filter["#h"]
            .as_array()
            .is_some_and(|values| values.len() == 1)));
    }
}

#[test]
fn workflow_query_results_are_deduplicated_by_event_id() {
    let first = wf_event(WF, CHAN, YAML);
    let second_workflow = "33333333-3333-3333-3333-333333333333";
    let second = wf_event(second_workflow, CHAN, YAML);
    let mut workflows = Vec::new();
    let mut seen_event_ids = HashSet::new();

    append_unique_workflows(
        &mut workflows,
        &mut seen_event_ids,
        &[first.clone(), second.clone()],
    );
    append_unique_workflows(&mut workflows, &mut seen_event_ids, &[first, second]);

    assert_eq!(workflows.len(), 2);
    assert_eq!(workflows[0].id, WF);
    assert_eq!(workflows[1].id, second_workflow);
}

#[test]
fn channel_workflow_filters_reject_malformed_or_blank_channel_ids() {
    for channel_id in ["not-a-uuid", "", "   "] {
        let error = channel_workflow_filters(vec![channel_id.to_string()])
            .expect_err("malformed channel id must fail before querying the relay");
        assert_eq!(error, "invalid channel id");
    }
}

#[test]
fn channel_workflow_filters_accepts_empty_input() {
    assert_eq!(
        channel_workflow_filters(Vec::new()).expect("empty input is valid"),
        Vec::<Value>::new()
    );
}

#[test]
fn trigger_response_uses_persisted_run_id_contract() {
    let wire = trigger_wire_from_message(
        WF.to_string(),
        "response:{\"run_id\":\"33333333-3333-3333-3333-333333333333\"}",
    )
    .expect("parse trigger response");

    assert_eq!(wire.run_id, "33333333-3333-3333-3333-333333333333");
    assert_eq!(wire.workflow_id, WF);
    assert_eq!(wire.status, "pending");
    let value = serde_json::to_value(wire).expect("serialize trigger response");
    assert!(value.get("event_id").is_none());
}

#[test]
fn trigger_response_rejects_missing_or_empty_run_id() {
    assert!(trigger_wire_from_message(WF.to_string(), "response:{}").is_err());
    assert!(trigger_wire_from_message(WF.to_string(), "response:{\"run_id\":\"   \"}",).is_err());
}

#[test]
fn run_reads_serialize_to_backend_envelopes() {
    let runs = WorkflowRunsWire {
        runs: Vec::new(),
        next: None,
    };
    let approvals = WorkflowApprovalsWire {
        approvals: Vec::new(),
    };
    assert_eq!(
        serde_json::to_value(runs).expect("serialize runs"),
        serde_json::json!({ "runs": [], "next": null })
    );
    assert_eq!(
        serde_json::to_value(approvals).expect("serialize approvals"),
        serde_json::json!({ "approvals": [] })
    );
}
