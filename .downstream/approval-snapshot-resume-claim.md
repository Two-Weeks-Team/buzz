# Typed approval snapshot and resume claim — candidate

Suspended executor results now carry the exact definition/trigger and resolved
gate (step, approver, message, timeout). The typed decoder requires the versioned
DB wrapper and validates the definition and gate binding; it does not turn old
or invalid context into defaults. A relay resume uses that snapshot, not the
latest editable definition, and rechecks current owner authority and the
channel/run/step binding.

`execute_from_step` preserves the stored trace and uses a DB compare-and-set
claim requiring a granted approval at exactly the waiting step. No two workers
can claim the same waiting state. Pre-claim failures use `ResumeNotClaimed` so
the relay does not finalize another worker's run as failed. Capacity denial
also preserves the durable waiting state.

Evidence: workflow unit suite 170 passed / 2 ignored; the actual PostgreSQL
suspension regression additionally verifies ungranted/wrong-step/foreign
claims, two concurrent contenders with exactly one winner, and trace/context
preservation. The schema decoder test binds the production snapshot builder.

Still incomplete: the finalizer retains `approval_not_supported`, so this is
not a claim that native approvals are enabled. Wire atomic suspension only
alongside a durable granted-run recovery queue (the current grant path merely
spawns a task), expiry/denial handling and emitted history. Crash recovery after
a run has been claimed must never blindly replay external effects. An old
snapshot-less gate is deliberately not automatically resumed by this candidate.
