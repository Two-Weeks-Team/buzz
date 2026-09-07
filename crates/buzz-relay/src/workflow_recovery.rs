//! Durable recovery of granted gates whose post-commit notification was lost.
//! Running claims are deliberately excluded: replaying them could duplicate
//! external effects. This loop does not grant business authority.

use std::sync::Arc;

/// Poll stored granted/waiting gates after the action sink has been initialized.
/// Each bounded page finishes before another is started; failures remain in the
/// database for the next scan. Invalid early rows cannot starve later pages.
pub async fn run(engine: Arc<buzz_workflow::WorkflowEngine>, db: buzz_db::Db) {
    let mut cursor = None;
    loop {
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
