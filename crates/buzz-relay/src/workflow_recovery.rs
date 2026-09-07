//! Durable recovery of granted gates whose post-commit notification was lost.
//! Running claims are deliberately excluded: replaying them could duplicate
//! external effects. This loop does not grant business authority.

use std::sync::Arc;

/// Recover only never-claimed dispatches using their original versioned inputs.
/// A separate bounded worker prevents approval execution from blocking this scan.
pub async fn run_pending(engine: Arc<buzz_workflow::WorkflowEngine>, db: buzz_db::Db) {
    let mut cursor = None;
    loop {
        match db.list_pending_workflow_starts(cursor).await {
            Ok(rows) => {
                cursor = rows
                    .last()
                    .map(|row| (*row.community_id.as_uuid(), row.run_id));
                let mut tasks = tokio::task::JoinSet::new();
                for row in rows {
                    let engine = Arc::clone(&engine);
                    let db = db.clone();
                    tasks.spawn(async move { recover_pending(&engine, &db, row).await });
                }
                while let Some(result) = tasks.join_next().await {
                    if let Err(error) = result {
                        tracing::error!("Pending workflow recovery task failed: {error}");
                    }
                }
            }
            Err(error) => tracing::error!("Pending workflow recovery scan failed: {error}"),
        }
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}

async fn recover_pending(
    engine: &buzz_workflow::WorkflowEngine,
    db: &buzz_db::Db,
    row: buzz_db::workflow::PendingWorkflowStart,
) {
    let result = pending_inputs(engine, db, &row).await;
    let snapshot = match result {
        Ok(snapshot) => snapshot,
        Err(error) => {
            tracing::warn!(run_id=%row.run_id, "Pending recovery refused: {error}");
            return;
        }
    };
    // execute_run owns the capacity permit and atomic Pending claim. Losing the
    // race to normal dispatch or another scanner must not finalize their run.
    let result = buzz_workflow::executor::execute_run(
        engine,
        row.community_id,
        row.run_id,
        &snapshot.definition,
        &snapshot.trigger,
    )
    .await;
    engine
        .finalize_run(row.community_id, row.run_id, result, None)
        .await;
}

async fn pending_inputs(
    engine: &buzz_workflow::WorkflowEngine,
    db: &buzz_db::Db,
    row: &buzz_db::workflow::PendingWorkflowStart,
) -> Result<buzz_workflow::snapshot::InitialExecutionSnapshot, String> {
    let run = db
        .get_workflow_run(row.community_id, row.run_id)
        .await
        .map_err(|e| e.to_string())?;
    if run.status != buzz_db::workflow::RunStatus::Pending
        || run.workflow_id != row.workflow_id
        || run.current_step != 0
        || run.execution_trace != serde_json::json!([])
    {
        return Err("not an untouched Pending run".into());
    }
    let snapshot = buzz_workflow::snapshot::InitialExecutionSnapshot::from_stored(
        run.trigger_context
            .as_ref()
            .ok_or("missing original snapshot")?,
    )
    .map_err(|e| e.to_string())?;
    let workflow = db
        .get_workflow(row.community_id, row.workflow_id)
        .await
        .map_err(|e| e.to_string())?;
    if !workflow.enabled
        || workflow.status != buzz_db::workflow::WorkflowStatus::Active
        || snapshot.workflow_id != row.workflow_id
        || workflow.channel_id != Some(snapshot.channel_id)
        || !snapshot
            .owner_pubkey
            .eq_ignore_ascii_case(&hex::encode(&workflow.owner_pubkey))
    {
        return Err("original/current workflow scope mismatch or inactive workflow".into());
    }
    engine
        .check_owner_authority(
            row.community_id,
            snapshot.channel_id,
            &workflow.owner_pubkey,
            &snapshot.definition,
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(snapshot)
}

#[cfg(test)]
mod postgres_tests {
    use super::*;
    use buzz_db::workflow::{PendingWorkflowStart, RunStatus};
    use uuid::Uuid;

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn pending_recovery_uses_original_inputs_and_current_authority() {
        let url = std::env::var("BUZZ_TEST_DATABASE_URL").expect("explicit test database");
        let pool = sqlx::PgPool::connect(&url).await.unwrap();
        let db = buzz_db::Db::from_pool(pool.clone());
        let community = db
            .ensure_configured_community(&format!("pending-{}.example", Uuid::new_v4()))
            .await
            .unwrap()
            .id;
        let owner = nostr::Keys::generate().public_key().to_bytes();
        db.ensure_user(community, &owner).await.unwrap();
        let channel = Uuid::new_v4();
        db.create_channel_with_id(
            community,
            channel,
            "pending-test",
            buzz_db::channel::ChannelType::Stream,
            buzz_db::channel::ChannelVisibility::Open,
            None,
            &owner,
            None,
        )
        .await
        .unwrap();
        let (definition, json) = buzz_workflow::WorkflowEngine::parse_yaml(
            "name: original\ntrigger:\n  on: message_posted\nsteps:\n  - id: gate\n    action: request_approval\n    from: any\n    message: 'Review {{trigger.text}}'\n    timeout: 1h\n").unwrap();
        let workflow = db
            .create_workflow(
                community,
                Some(channel),
                &owner,
                "original",
                &json,
                &[7; 32],
            )
            .await
            .unwrap();
        let trigger = buzz_workflow::executor::TriggerContext {
            channel_id: channel.to_string(),
            text: "original input".into(),
            ..Default::default()
        };
        let snapshot = buzz_workflow::snapshot::InitialExecutionSnapshot::capture(
            workflow,
            channel,
            &owner,
            &definition,
            &trigger,
        )
        .unwrap();
        let run = db
            .create_workflow_run(community, workflow, None, Some(&snapshot))
            .await
            .unwrap();
        let row = || PendingWorkflowStart {
            community_id: community,
            run_id: run,
            workflow_id: workflow,
        };
        let engine = buzz_workflow::WorkflowEngine::new(db.clone(), Default::default());
        // Latest definition is deliberately different; recovery must not use it.
        sqlx::query("UPDATE workflows SET definition=$1 WHERE community_id=$2 AND id=$3")
            .bind(serde_json::json!({"name":"edited","trigger":{"on":"message_posted"},"steps":[]}))
            .bind(community.as_uuid())
            .bind(workflow)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            pending_inputs(&engine, &db, &row())
                .await
                .unwrap()
                .definition
                .name,
            "original"
        );
        for invalid in [
            serde_json::json!({"text":"legacy"}),
            {
                let mut value = snapshot.clone();
                value["initial"]["workflow_id"] = serde_json::json!(Uuid::new_v4());
                value
            },
            {
                let mut value = snapshot.clone();
                value["initial"]["owner_pubkey"] = serde_json::json!(hex::encode([9; 32]));
                value
            },
        ] {
            sqlx::query(
                "UPDATE workflow_runs SET trigger_context=$1 WHERE community_id=$2 AND id=$3",
            )
            .bind(invalid)
            .bind(community.as_uuid())
            .bind(run)
            .execute(&pool)
            .await
            .unwrap();
            recover_pending(&engine, &db, row()).await;
            assert_eq!(
                db.get_workflow_run(community, run).await.unwrap().status,
                RunStatus::Pending
            );
        }
        sqlx::query("UPDATE workflow_runs SET trigger_context=$1 WHERE community_id=$2 AND id=$3")
            .bind(&snapshot)
            .bind(community.as_uuid())
            .bind(run)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE workflows SET enabled=false WHERE community_id=$1 AND id=$2")
            .bind(community.as_uuid())
            .bind(workflow)
            .execute(&pool)
            .await
            .unwrap();
        assert!(pending_inputs(&engine, &db, &row()).await.is_err());
        sqlx::query("UPDATE workflows SET enabled=true WHERE community_id=$1 AND id=$2")
            .bind(community.as_uuid())
            .bind(workflow)
            .execute(&pool)
            .await
            .unwrap();
        tokio::join!(
            recover_pending(&engine, &db, row()),
            recover_pending(&engine, &db, row())
        );
        let stored = db.get_workflow_run(community, run).await.unwrap();
        assert_eq!(stored.status, RunStatus::WaitingApproval);
        let gate = buzz_workflow::snapshot::ExecutionSnapshot::from_stored(
            stored.trigger_context.as_ref().unwrap(),
        )
        .unwrap();
        assert_eq!(gate.definition.name, "original");
        assert_eq!(gate.gate.message, "Review original input");
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM workflow_approvals WHERE community_id=$1 AND run_id=$2",
        )
        .bind(community.as_uuid())
        .bind(run)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
        let denied_run = db
            .create_workflow_run(community, workflow, None, Some(&snapshot))
            .await
            .unwrap();
        let successor = nostr::Keys::generate().public_key().to_bytes();
        db.ensure_user(community, &successor).await.unwrap();
        db.add_member(
            community,
            channel,
            &successor,
            buzz_db::channel::MemberRole::Owner,
            Some(&owner),
        )
        .await
        .unwrap();
        db.remove_member(community, channel, &owner, &owner)
            .await
            .unwrap();
        recover_pending(
            &engine,
            &db,
            PendingWorkflowStart {
                community_id: community,
                run_id: denied_run,
                workflow_id: workflow,
            },
        )
        .await;
        assert_eq!(
            db.get_workflow_run(community, denied_run)
                .await
                .unwrap()
                .status,
            RunStatus::Pending
        );
    }
}

/// Poll stored granted/waiting gates after the action sink has been initialized.
/// Each bounded page finishes before another is started; failures remain in the
/// database for the next scan. Invalid early rows cannot starve later pages.
pub async fn run(engine: Arc<buzz_workflow::WorkflowEngine>, db: buzz_db::Db) {
    let mut cursor = None;
    loop {
        if let Err(error) = db.expire_workflow_approvals().await {
            // Keep the durable pending gates for a later bounded retry.
            tracing::error!("Workflow approval expiry failed: {error}");
        }
        let rows = match db.list_granted_workflow_resumes(cursor).await {
            Ok(rows) => rows,
            Err(error) => {
                tracing::error!("Workflow approval recovery scan failed: {error}");
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                continue;
            }
        };
        cursor = rows
            .last()
            .map(|row| (*row.community_id.as_uuid(), row.run_id));
        let mut pending = tokio::task::JoinSet::new();
        for row in rows {
            let Some(next_step) = usize::try_from(row.step_index)
                .ok()
                .and_then(|step| step.checked_add(1))
            else {
                tracing::error!(run_id=%row.run_id, "Invalid durable approval step");
                continue;
            };
            let engine = Arc::clone(&engine);
            let db = db.clone();
            pending.spawn(async move {
                crate::handlers::command_executor::resume_workflow_after_approval(
                    engine,
                    db,
                    row.community_id,
                    row.run_id,
                    row.workflow_id,
                    next_step,
                )
                .await;
            });
        }
        while let Some(result) = pending.join_next().await {
            if let Err(error) = result {
                // Never blindly retry a potentially claimed run after panic.
                // The next scan only selects still-waiting rows.
                tracing::error!("Workflow approval recovery task failed: {error}");
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}
