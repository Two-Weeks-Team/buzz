import type { WorkflowApproval, WorkflowRun } from "@/shared/api/types";

export type ApprovalActorScope = { relayUrl: string; pubkey: string };
export type ApprovalDecisionAction = "grant" | "deny";
export type SubmitApprovalDecision = (
  approval: WorkflowApproval,
  action: ApprovalDecisionAction,
  note: string,
) => Promise<void>;

/** UI eligibility only; native preflight and relay authorization remain authoritative. */
export function canDecideApproval(
  run: WorkflowRun,
  approval: WorkflowApproval,
  scope: ApprovalActorScope | undefined,
  now: number,
): boolean {
  if (!scope?.relayUrl.trim() || !/^[0-9a-f]{64}$/i.test(scope.pubkey))
    return false;
  const spec = approval.approverSpec.trim();
  return (
    run.status === "waiting_approval" &&
    run.id === approval.runId &&
    run.workflowId === approval.workflowId &&
    run.currentStep === approval.stepIndex &&
    approval.status === "pending" &&
    /^[0-9a-f]{64}$/i.test(approval.approvalRef) &&
    Date.parse(approval.expiresAt) > now &&
    (spec === "any" ||
      spec === "" ||
      spec.toLowerCase() === scope.pubkey.toLowerCase())
  );
}
