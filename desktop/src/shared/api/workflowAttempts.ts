import { invokeTauri } from "./tauri";

export type AttemptScope = {
  workflowId: string;
  runId: string;
  expectedRelayUrl: string;
  expectedSignerPubkey: string;
};
export type StepAttempt = {
  stepIndex: number;
  epoch: number;
  stepId: string;
  digest: string;
  startedAt: string;
  returnedAt: string | null;
  result: Record<string, unknown> | null;
};
export type AttemptPage = { attempts: StepAttempt[]; next: number | null };

function object(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
function timestamp(value: unknown): value is string {
  return (
    typeof value === "string" &&
    /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?(?:Z|[+-]\d\d:\d\d)$/.test(
      value,
    ) &&
    Number.isFinite(Date.parse(value))
  );
}

/** Strict page decode: malformed/foreign data never becomes empty success. */
export function parseAttemptPage(
  raw: unknown,
  scope: Pick<AttemptScope, "workflowId" | "runId">,
  after: number | null,
): AttemptPage {
  if (
    !object(raw) ||
    raw.workflow_id !== scope.workflowId ||
    raw.run_id !== scope.runId ||
    !Array.isArray(raw.attempts) ||
    raw.attempts.length > 16
  )
    throw new Error("Journal response is unavailable or out of scope");
  let previous = after ?? -1;
  const attempts = raw.attempts.map((value: unknown): StepAttempt => {
    if (
      !object(value) ||
      !Number.isSafeInteger(value.step_index) ||
      (value.step_index as number) <= previous ||
      (value.step_index as number) >= 4096 ||
      !Number.isSafeInteger(value.execution_epoch) ||
      (value.execution_epoch as number) < 1 ||
      typeof value.step_id !== "string" ||
      !value.step_id ||
      value.step_id.length > 256 ||
      typeof value.action_digest !== "string" ||
      !/^[0-9a-f]{64}$/.test(value.action_digest) ||
      !timestamp(value.started_at) ||
      !(
        (value.returned_at === null && value.result === null) ||
        (timestamp(value.returned_at) && object(value.result))
      )
    )
      throw new Error("Journal contains an invalid attempt");
    previous = value.step_index as number;
    return {
      stepIndex: previous,
      epoch: value.execution_epoch as number,
      stepId: value.step_id,
      digest: value.action_digest,
      startedAt: value.started_at,
      returnedAt: value.returned_at as string | null,
      result: value.result as Record<string, unknown> | null,
    };
  });
  if (
    raw.next !== null &&
    (!object(raw.next) ||
      attempts.length === 0 ||
      raw.next.after_index !== previous)
  )
    throw new Error("Journal pagination is unavailable");
  return { attempts, next: raw.next === null ? null : previous };
}

export async function getWorkflowAttempts(
  scope: AttemptScope,
  after: number | null,
): Promise<AttemptPage> {
  const raw = await invokeTauri<unknown>("get_run_attempts", {
    request: { ...scope, afterIndex: after },
  });
  return parseAttemptPage(raw, scope, after);
}
