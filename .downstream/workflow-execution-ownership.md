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

An independent observer now inventories expired owned Running rows in keyset
pages of 32, then marks each uncertain only if token/epoch/deadline still match.
Per-row observation has a five-second timeout; pages are separated by 30 seconds.
It does not execute or requeue anything, and does not invent leases for legacy
rows. This is bounded work, not a fixed detection SLA.

Authenticated run history includes token-free `execution` observations: state,
epoch, lease_expires_at and DB observed_at. States are not_started, leased,
outcome_unknown, legacy_unowned, waiting_approval and settled. An expired owned
Running row reads as outcome_unknown even before the observer persists its flag.
Leased means only that its lease was valid at the observation time, not a proof
that the process is alive or making progress. The top-level DB run status remains
Running for uncertain executions; clients must present the observation too.

Remaining integration must persist pre-effect intent and incremental progress,
expose this observation in Desktop with refresh/staleness handling, and prove
actual process-crash lease expiry over signed transport. Do not claim the UI is
accurate until its conversion/display and real browser evidence are complete.
Do not interpret execution_uncertain=false on a legacy row as proven success or
proven absence of effects. The old APIs must be removed from runtime call sites
or reject token-owned runs; direct low-level action dispatch is not itself a
business-authorized workflow entrypoint. External provider idempotency is not
created by this local fencing mechanism.

No timeout can establish whether an external recipient committed an effect.
Lease loss must not trigger blind replay; scoped readonly observation and
independent reconciliation remain necessary. Business authorization is separate.
