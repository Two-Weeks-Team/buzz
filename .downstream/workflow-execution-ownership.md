# Workflow execution ownership candidate

Migration 0045 and desired schema add an opaque execution token, monotonic epoch,
lease deadline and uncertainty flag to existing workflow runs. Historical rows
are not assigned a fabricated owner or lease. No existing run is replayed.

The new database primitives atomically claim initial or exactly granted resume
execution with a 120-second lease; renew only an unexpired matching claim; fence
an expired claim as uncertain without erasing its evidence; and accept terminal
results only from the current live token/epoch. A resume advances the epoch, so
an earlier worker cannot renew or complete the resumed run through these APIs.

PostgreSQL tests cover competing initial claims, wrong tenant/token/epoch,
expiry before observation, idempotent uncertainty marking, preserved context,
terminal immutability, exact granted resume and generation rotation. The same
expiry/terminal test also runs after the complete embedded migration chain in
an empty isolated database, not just the desired schema.

Initial and resumed execution now carry the claim through ExecutionResult or
PartialProgress. A scoped select loop renews every 30 seconds while executing;
each step also checks/renews ownership before evaluating or dispatching it.
Loss cancels the remaining future and the finalizer conservatively marks the
matching run uncertain. Timeout/webhook transport errors also leave uncertainty,
not a false assertion that no effect happened. Gate suspension requires the
exact live claim in its gate+trace transaction; terminal writes use the same
fence. Old unowned start/resume/update/suspend APIs reject token-owned rows.

Remaining integration must persist pre-effect intent and incremental progress,
classify process-crash lease expiry, and expose uncertain/legacy-unowned outcomes
through authenticated history/UI. The DB run status currently remains Running
when its execution_uncertain flag is true; do not claim the UI is accurate yet.
Do not interpret execution_uncertain=false on a legacy row as proven success or
proven absence of effects. The old APIs must be removed from runtime call sites
or reject token-owned runs; direct low-level action dispatch is not itself a
business-authorized workflow entrypoint. External provider idempotency is not
created by this local fencing mechanism.

No timeout can establish whether an external recipient committed an effect.
Lease loss must not trigger blind replay; scoped readonly observation and
independent reconciliation remain necessary. Business authorization is separate.
