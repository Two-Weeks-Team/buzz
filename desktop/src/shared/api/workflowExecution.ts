import type {
  WorkflowExecutionObservation,
  WorkflowRun,
} from "./workflowTypes";

/** Decode observations only; missing/invalid data is never a live execution. */
export function parseExecutionObservation(
  raw: unknown,
): WorkflowExecutionObservation | null {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
  const value = raw as Record<string, unknown>;
  const states = [
    "not_started",
    "leased",
    "outcome_unknown",
    "legacy_unowned",
    "waiting_approval",
    "settled",
  ];
  if (
    typeof value.state !== "string" ||
    !states.includes(value.state) ||
    !Number.isSafeInteger(value.epoch) ||
    (value.epoch as number) < 0 ||
    typeof value.observed_at !== "string" ||
    !Number.isFinite(Date.parse(value.observed_at)) ||
    (value.lease_expires_at !== null &&
      (typeof value.lease_expires_at !== "string" ||
        !Number.isFinite(Date.parse(value.lease_expires_at))))
  )
    return null;
  if (
    value.state === "leased" &&
    ((value.epoch as number) < 1 ||
      value.lease_expires_at === null ||
      Date.parse(value.lease_expires_at as string) <=
        Date.parse(value.observed_at))
  )
    return null;
  return {
    state: value.state as WorkflowExecutionObservation["state"],
    epoch: value.epoch as number,
    leaseExpiresAt: value.lease_expires_at as string | null,
    observedAt: value.observed_at,
  };
}

/** An observed lease is not a promise of process liveness or safe replay. */
export function executionPresentation(
  run: Pick<WorkflowRun, "status" | "execution">,
  now: number,
) {
  if (run.status !== "running")
    return { label: run.status.replaceAll("_", " "), detail: null };
  const observation = run.execution;
  if (!observation)
    return {
      label: "Execution state unavailable",
      detail:
        "Ownership information is unavailable. Refresh history; do not assume the work can be retried.",
    };
  if (observation.state === "outcome_unknown")
    return {
      label: "Outcome unknown",
      detail:
        "The execution may have produced effects before it stopped. It has not been automatically replayed. Inspect evidence before recovery.",
    };
  if (observation.state === "legacy_unowned")
    return {
      label: "Execution unverified",
      detail:
        "This run has no verifiable execution owner. Its effects and progress are not established by the running status.",
    };
  if (observation.state !== "leased")
    return {
      label: "Execution state unavailable",
      detail:
        "The execution observation does not match this run. Refresh history.",
    };
  const observed = Date.parse(observation.observedAt);
  const age = now - observed;
  if (
    age < -5000 ||
    age > 30000 ||
    now >= Date.parse(observation.leaseExpiresAt ?? "")
  ) {
    return {
      label: "Refresh execution state",
      detail:
        "The lease observation is stale or the clocks differ. Refresh history to obtain the relay's current observation.",
    };
  }
  return {
    label: "Lease observed",
    detail:
      "The relay observed a valid execution lease. This does not prove the worker is alive, progressing, or that an external effect succeeded.",
  };
}
