//! Versioned execution state captured at an approval gate. Never reconstruct a
//! suspended run from the workflow's latest editable definition or empty input.

use crate::{executor::TriggerContext, schema::ActionDef, WorkflowDef, WorkflowError};
use serde::{Deserialize, Serialize};

/// Resolved approval fields, bound to the suspended definition's step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalGate {
    /// Zero-based index of the approval action.
    pub step_index: usize,
    /// Stable step identifier.
    pub step_id: String,
    /// Resolved approver specification (not raw template text).
    pub from: String,
    /// Resolved question shown to the approver.
    pub message: String,
    /// Approval window in seconds.
    pub timeout_secs: u64,
}

/// Immutable inputs for the remaining execution, not an authority grant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionSnapshot {
    /// Exact definition used by the running executor.
    pub definition: WorkflowDef,
    /// Original typed trigger data, including webhook fields.
    pub trigger: TriggerContext,
    /// Resolved suspended gate.
    pub gate: ApprovalGate,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredSnapshot {
    buzz_execution_version: u32,
    snapshot: ExecutionSnapshot,
}

impl ExecutionSnapshot {
    /// Validate before persistence and after decoding. Legacy/malformed snapshots
    /// fail closed; no caller may silently upgrade them with the current definition.
    pub fn validate(&self) -> Result<(), WorkflowError> {
        self.definition.validate()?;
        let step = self
            .definition
            .steps
            .get(self.gate.step_index)
            .ok_or_else(|| {
                WorkflowError::InvalidDefinition("snapshot gate index out of range".into())
            })?;
        if step.id != self.gate.step_id
            || !matches!(step.action, ActionDef::RequestApproval { .. })
            || self.gate.from.trim().is_empty()
            || self.gate.timeout_secs == 0
        {
            return Err(WorkflowError::InvalidDefinition(
                "snapshot gate mismatch".into(),
            ));
        }
        Ok(())
    }

    /// Decode the DB's versioned wrapper and require a complete valid snapshot.
    pub fn from_stored(value: &serde_json::Value) -> Result<Self, WorkflowError> {
        let stored: StoredSnapshot = serde_json::from_value(value.clone()).map_err(|_| {
            WorkflowError::InvalidDefinition("missing or invalid execution snapshot".into())
        })?;
        if stored.buzz_execution_version != 1 {
            return Err(WorkflowError::InvalidDefinition(
                "unsupported execution snapshot version".into(),
            ));
        }
        stored.snapshot.validate()?;
        Ok(stored.snapshot)
    }
}
