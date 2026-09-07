import * as React from "react";
import type { WorkflowRun } from "@/shared/api/workflowTypes";
import { executionPresentation } from "@/shared/api/workflowExecution";
import { Button } from "@/shared/ui/button";

export function WorkflowExecutionNotice({
  run,
  onRefresh,
  refreshing = false,
}: {
  run: WorkflowRun;
  onRefresh: () => void;
  refreshing?: boolean;
}) {
  const [now, setNow] = React.useState(Date.now);
  React.useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);
  const presentation = executionPresentation(run, now);
  if (run.status !== "running") return null;
  return (
    <div
      className="space-y-2 rounded-md border p-3 text-xs"
      data-testid="workflow-execution-observation"
    >
      <p className="font-medium">{presentation.label}</p>
      <p className="text-muted-foreground">{presentation.detail}</p>
      {run.execution ? (
        <p className="break-words font-mono text-2xs">
          Generation {run.execution.epoch} · Observed {run.execution.observedAt}
          {run.execution.leaseExpiresAt
            ? ` · Lease until ${run.execution.leaseExpiresAt}`
            : ""}
        </p>
      ) : null}
      <Button
        type="button"
        size="sm"
        variant="outline"
        disabled={refreshing}
        onClick={onRefresh}
      >
        Refresh history
      </Button>
    </div>
  );
}
