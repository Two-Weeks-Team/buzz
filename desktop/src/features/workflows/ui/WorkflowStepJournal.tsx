import * as React from "react";
import {
  getWorkflowAttempts,
  type AttemptPage,
  type AttemptScope,
} from "@/shared/api/workflowAttempts";
import { Button } from "@/shared/ui/button";
import { useWorkflowHistoryCapability } from "./useWorkflowHistoryCapability";

/** Mount keyed by relay/signer/workflow/run. Read failures never imply no effects. */
export function WorkflowStepJournal({ scope }: { scope: AttemptScope }) {
  const supported = useWorkflowHistoryCapability(
    true,
    "buzz.workflow-attempts.v1",
  );
  const [cursors, setCursors] = React.useState<(number | null)[]>([null]);
  const [refresh, setRefresh] = React.useState(0);
  const [page, setPage] = React.useState<AttemptPage | null>(null);
  const [loading, setLoading] = React.useState(false);
  const [failed, setFailed] = React.useState(false);
  const after = cursors[cursors.length - 1] ?? null;
  const { workflowId, runId, expectedRelayUrl, expectedSignerPubkey } = scope;
  React.useEffect(() => {
    let current = true;
    setPage(null);
    setFailed(false);
    if (!supported) {
      setLoading(false);
      return;
    }
    setLoading(true);
    void getWorkflowAttempts(
      { workflowId, runId, expectedRelayUrl, expectedSignerPubkey },
      after,
    )
      .then((result) => {
        if (current) setPage(result);
      })
      .catch(() => {
        if (current) setFailed(true);
      })
      .finally(() => {
        if (current) setLoading(false);
      });
    return () => {
      current = false;
    };
  }, [
    supported,
    workflowId,
    runId,
    expectedRelayUrl,
    expectedSignerPubkey,
    after,
    refresh,
  ]);
  return (
    <section
      className="my-3 space-y-2 rounded-md border p-3 text-xs"
      aria-label="Durable step journal"
    >
      <h4 className="font-medium">Durable step journal</h4>
      <p className="text-muted-foreground">
        Attempts and recorded returns are evidence, not proof of external
        success or permission to retry.
      </p>
      {!supported ? (
        <p>Journal capability is not advertised or is unavailable.</p>
      ) : (
        <>
          <Button
            size="sm"
            variant="outline"
            disabled={loading}
            onClick={() => {
              setPage(null);
              setCursors([null]);
              setRefresh((value) => value + 1);
            }}
          >
            Refresh journal
          </Button>
          {loading ? <p role="status">Reading journal…</p> : null}
          {failed ? (
            <p role="alert">
              Journal unavailable. Refresh to try reading again.
            </p>
          ) : null}
          {page?.attempts.length === 0 ? (
            <p>
              No attempt records on this page. Legacy or missing records do not
              prove that no work occurred.
            </p>
          ) : null}
          {page?.attempts.map((attempt) => (
            <article
              className="space-y-1 border-t pt-2"
              key={attempt.stepIndex}
            >
              <h5 className="font-medium">
                Step {attempt.stepIndex + 1}: {attempt.stepId} · generation{" "}
                {attempt.epoch}
              </h5>
              <p>
                {attempt.returnedAt === null
                  ? "Return unconfirmed — effects may have occurred."
                  : "Executor return recorded — verify external effects separately."}
              </p>
              <p className="break-all font-mono text-2xs">
                Intent {attempt.startedAt}
                {attempt.returnedAt ? ` · Return ${attempt.returnedAt}` : ""}
              </p>
              <details>
                <summary className="cursor-pointer">Recorded evidence</summary>
                <p className="break-all font-mono text-2xs">
                  SHA-256 {attempt.digest}
                </p>
                {attempt.result ? (
                  <pre className="max-h-40 overflow-auto whitespace-pre-wrap break-words text-2xs">
                    {JSON.stringify(attempt.result, null, 2)}
                  </pre>
                ) : null}
              </details>
            </article>
          ))}
          <div className="flex gap-2">
            {cursors.length > 1 ? (
              <Button
                size="sm"
                variant="outline"
                disabled={loading}
                onClick={() => {
                  setPage(null);
                  setCursors((value) => value.slice(0, -1));
                }}
              >
                Previous attempts
              </Button>
            ) : null}
            {page?.next != null ? (
              <Button
                size="sm"
                variant="outline"
                disabled={loading}
                onClick={() => {
                  const next = page.next;
                  setPage(null);
                  setCursors((value) => [...value, next]);
                }}
              >
                Next attempts
              </Button>
            ) : null}
          </div>
        </>
      )}
    </section>
  );
}
