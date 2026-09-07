import type { WorkflowApproval } from "@/shared/api/types";
import * as React from "react";
import { Button } from "@/shared/ui/button";
import { Textarea } from "@/shared/ui/textarea";
import type {
  ApprovalActorScope,
  ApprovalDecisionAction,
  SubmitApprovalDecision,
} from "./approvalAction";

type WorkflowApprovalCardProps = {
  approval: WorkflowApproval;
  question?: string | null;
  scope?: ApprovalActorScope;
  onDecision?: SubmitApprovalDecision;
  onRefresh?: () => void;
  busy?: boolean;
};

export function WorkflowApprovalCard({
  approval,
  question,
  scope,
  onDecision,
  onRefresh,
  busy,
}: WorkflowApprovalCardProps) {
  const [action, setAction] = React.useState<ApprovalDecisionAction | null>(
    null,
  );
  const [note, setNote] = React.useState("");
  const [outcome, setOutcome] = React.useState<
    "idle" | "sending" | "recorded" | "unknown"
  >("idle");
  const locked = React.useRef(false);
  const [now, setNow] = React.useState(Date.now);
  React.useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);
  const isExpired = !(Date.parse(approval.expiresAt) > now);

  async function submit() {
    if (
      !action ||
      !onDecision ||
      !question?.trim() ||
      locked.current ||
      busy ||
      isExpired
    )
      return;
    locked.current = true;
    setOutcome("sending");
    try {
      await onDecision(approval, action, note);
      setOutcome("recorded");
    } catch {
      // A lost response may hide a committed decision. Never re-enable or retry
      // automatically; authoritative history decides what happened.
      setOutcome("unknown");
    }
  }

  if (approval.status !== "pending") {
    return null;
  }

  return (
    <div
      className="rounded-lg border border-amber-500/30 bg-amber-500/5 p-3"
      data-testid="workflow-approval-card"
    >
      <p className="mb-2 text-sm font-medium">Approval Required</p>
      <p className="mb-2 break-all text-xs text-muted-foreground">
        Approver: {approval.approverSpec}
      </p>
      <p className="mb-2 text-xs text-muted-foreground">
        Expires: {new Date(approval.expiresAt).toLocaleString()}
      </p>
      {outcome !== "idle" ? (
        <div className="space-y-2 text-xs" role="status">
          <p>
            {outcome === "sending"
              ? "Submitting signed decision…"
              : outcome === "recorded"
                ? "Decision recorded. Refresh history to see execution progress."
                : "Decision outcome is unconfirmed. It may already be recorded. Refresh history before taking any further action."}
          </p>
          {outcome !== "sending" && onRefresh ? (
            <Button size="sm" variant="outline" onClick={onRefresh}>
              Refresh history
            </Button>
          ) : null}
        </div>
      ) : isExpired ? (
        <p className="text-xs text-muted-foreground" role="status">
          Approval deadline passed or is unavailable. Refresh history for the
          server decision.
        </p>
      ) : !onDecision || !scope || !question?.trim() ? (
        <p className="text-xs text-muted-foreground" role="status">
          This gate requires another approver, or its current scope or request
          text is unavailable.
        </p>
      ) : action ? (
        <div className="space-y-2 text-xs">
          <p className="font-medium">
            Confirm {action === "grant" ? "approval" : "denial"} of this gate
            only
          </p>
          <p className="whitespace-pre-wrap break-words font-medium">
            {question}
          </p>
          <p className="break-all">Relay: {scope.relayUrl}</p>
          <p className="break-all">Signer: {scope.pubkey}</p>
          <p className="break-all">Workflow: {approval.workflowId}</p>
          <p className="break-all">
            Run: {approval.runId} · Step: {approval.stepId}
          </p>
          <p className="break-all">Reference: {approval.approvalRef}</p>
          <Textarea
            aria-label="Decision note"
            placeholder="Optional decision note"
            value={note}
            onChange={(event) => setNote(event.target.value)}
            disabled={busy}
          />
          <p>
            Approval permits the workflow to continue; it does not confirm
            successful execution or grant separate business authority.
          </p>
          <div className="flex gap-2">
            <Button
              size="sm"
              variant={action === "deny" ? "destructive" : "default"}
              disabled={busy}
              onClick={() => void submit()}
            >
              Confirm {action === "grant" ? "approve" : "deny"}
            </Button>
            <Button
              size="sm"
              variant="outline"
              disabled={busy}
              onClick={() => setAction(null)}
            >
              Cancel
            </Button>
          </div>
        </div>
      ) : (
        <div className="flex gap-2">
          <Button size="sm" disabled={busy} onClick={() => setAction("grant")}>
            Review approval
          </Button>
          <Button
            size="sm"
            variant="outline"
            disabled={busy}
            onClick={() => setAction("deny")}
          >
            Review denial
          </Button>
        </div>
      )}
    </div>
  );
}
