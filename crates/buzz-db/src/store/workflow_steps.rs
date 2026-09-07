//! Durable step attempts, separate from the finalizer's execution trace.
//! A recorded return is executor evidence, not independent provider reconciliation.

#[cfg(test)]
#[path = "workflow_steps_postgres_tests.rs"]
mod postgres_tests;

use buzz_core::CommunityId;
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::{workflow::WorkflowExecutionClaim, Db, DbError, Result};

/// Token-free evidence for one attempted step; a return is not provider proof.
#[derive(Debug, serde::Serialize)]
pub struct WorkflowStepAttempt {
    /// Zero-based immutable step index.
    pub step_index: i32,
    /// Execution generation that recorded the intent.
    pub execution_epoch: i64,
    /// Definition's step identifier.
    pub step_id: String,
    /// SHA-256 digest, hex encoded, of the executor's serialized intent.
    pub action_digest: String,
    /// Database time of the intent insert (the write is committed before dispatch).
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// Database observation time of an executor return, absent if unconfirmed.
    pub returned_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Recorded executor result, never inferred from the run status.
    pub result: Option<Value>,
}

async fn lock_execution(
    tx: &mut Transaction<'_, Postgres>,
    community: CommunityId,
    run: Uuid,
) -> Result<()> {
    sqlx::query("SET LOCAL lock_timeout='1s'")
        .execute(&mut **tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut **tx)
        .await?;
    // Lock first, then evaluate live lease in a fresh statement after any wait.
    sqlx::query("SELECT id FROM workflow_runs WHERE community_id=$1 AND id=$2 FOR UPDATE")
        .bind(community.as_uuid())
        .bind(run)
        .fetch_optional(&mut **tx)
        .await?;
    Ok(())
}

impl Db {
    /// Read a bounded ascending journal page with explicit tenant/workflow/run
    /// binding. The extra (33rd) row permits a maximum-32 API page's lookahead.
    pub async fn list_workflow_step_attempts(
        &self,
        community: CommunityId,
        workflow: Uuid,
        run: Uuid,
        after_index: Option<i32>,
        limit: i64,
    ) -> Result<Vec<WorkflowStepAttempt>> {
        let after = after_index.unwrap_or(-1);
        if !(-1..4096).contains(&after) || !(1..=33).contains(&limit) {
            return Err(DbError::InvalidData("invalid step journal page".into()));
        }
        let rows=sqlx::query("SELECT a.step_index,a.execution_epoch,a.step_id,encode(a.action_digest,'hex') AS action_digest,a.started_at,a.returned_at,a.result
            FROM workflow_step_attempts a JOIN workflow_runs r ON r.community_id=a.community_id AND r.id=a.run_id
            WHERE a.community_id=$1 AND r.workflow_id=$2 AND a.run_id=$3 AND a.step_index>$4
            ORDER BY a.step_index ASC LIMIT $5")
            .bind(community.as_uuid()).bind(workflow).bind(run).bind(after).bind(limit).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(WorkflowStepAttempt {
                    step_index: row.try_get("step_index")?,
                    execution_epoch: row.try_get("execution_epoch")?,
                    step_id: row.try_get("step_id")?,
                    action_digest: row.try_get("action_digest")?,
                    started_at: row.try_get("started_at")?,
                    returned_at: row.try_get("returned_at")?,
                    result: row.try_get("result")?,
                })
            })
            .collect()
    }

    /// Commit one step intent before dispatch. Duplicate attempts, including a
    /// later epoch, are denied; callers must not dispatch when false or uncertain.
    /// Store a SHA-256 digest of resolved action bytes, not secret-bearing inputs.
    #[allow(clippy::too_many_arguments)]
    pub async fn begin_workflow_step_attempt(
        &self,
        community: CommunityId,
        run: Uuid,
        claim: WorkflowExecutionClaim,
        index: i32,
        step_id: &str,
        action_digest: &[u8],
    ) -> Result<bool> {
        if !(0..4096).contains(&index)
            || step_id.is_empty()
            || step_id.len() > 256
            || action_digest.len() != 32
        {
            return Err(DbError::InvalidData("invalid workflow step intent".into()));
        }
        let mut tx = self.pool.begin().await?;
        lock_execution(&mut tx, community, run).await?;
        let changed=sqlx::query(
            "INSERT INTO workflow_step_attempts (community_id,run_id,step_index,execution_epoch,step_id,action_digest)
             SELECT community_id,id,$5,$4,$6,$7 FROM workflow_runs
             WHERE community_id=$1 AND id=$2 AND status='running' AND current_step=$5
               AND execution_token=$3 AND execution_epoch=$4 AND NOT execution_uncertain
               AND execution_lease_until>clock_timestamp()
             ON CONFLICT (community_id,run_id,step_index) DO NOTHING"
        ).bind(community.as_uuid()).bind(run).bind(claim.token).bind(claim.epoch)
            .bind(index).bind(step_id).bind(action_digest).execute(&mut *tx).await?.rows_affected();
        tx.commit().await?;
        Ok(changed == 1)
    }

    /// Record the exact attempt's returned result once and advance its current
    /// step atomically when requested. Suspension keeps the gate's step index.
    /// The result must be bounded JSON; this API does not infer business success.
    #[allow(clippy::too_many_arguments)]
    pub async fn record_workflow_step_return(
        &self,
        community: CommunityId,
        run: Uuid,
        claim: WorkflowExecutionClaim,
        index: i32,
        result: &Value,
        advance: bool,
    ) -> Result<bool> {
        if !(0..4096).contains(&index) || !result.is_object() || result.to_string().len() > 32768 {
            return Err(DbError::InvalidData("invalid workflow step result".into()));
        }
        let mut tx = self.pool.begin().await?;
        lock_execution(&mut tx, community, run).await?;
        let changed = sqlx::query(
            "UPDATE workflow_step_attempts a SET returned_at=clock_timestamp(),result=$6
             FROM workflow_runs r WHERE a.community_id=$1 AND a.run_id=$2 AND a.step_index=$5
               AND a.execution_epoch=$4 AND a.returned_at IS NULL
               AND r.community_id=a.community_id AND r.id=a.run_id
               AND r.status='running' AND r.current_step=$5 AND r.execution_token=$3
               AND r.execution_epoch=$4 AND NOT r.execution_uncertain
               AND r.execution_lease_until>clock_timestamp()",
        )
        .bind(community.as_uuid())
        .bind(run)
        .bind(claim.token)
        .bind(claim.epoch)
        .bind(index)
        .bind(result)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if changed == 1 && advance {
            sqlx::query("UPDATE workflow_runs SET current_step=$3 WHERE community_id=$1 AND id=$2")
                .bind(community.as_uuid())
                .bind(run)
                .bind(index + 1)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(changed == 1)
    }
}
