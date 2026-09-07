import type {
  TraceEntry,
  WorkflowApproval,
  WorkflowRun,
} from "@/shared/api/types";

/** Only the same persisted gate may supply a decision for a historical trace. */
export function approvalForTrace(
  run: WorkflowRun,
  step: TraceEntry,
  approvals: WorkflowApproval[],
): WorkflowApproval | undefined {
  if (!step.approvalRef) return undefined;
  return approvals.find(
    (approval) =>
      approval.approvalRef === step.approvalRef &&
      approval.workflowId === run.workflowId &&
      approval.runId === run.id &&
      approval.stepId === step.stepId,
  );
}

/** Request history alone is not evidence of the current approval status. */
export function approvalTraceStatus(
  step: TraceEntry,
  approval?: WorkflowApproval,
): string {
  if (step.status !== "waiting_approval") return step.status;
  return approval?.status ?? "approval_requested";
}
