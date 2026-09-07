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

This is a prerequisite, NOT runtime ownership enforcement yet. The existing
executor/approval suspension/finalizer still use the older APIs. Integration
must carry the claim through every step and gate/terminal transaction, renew
while executing, persist pre-effect intent and progress, cancel on lease loss,
and expose uncertain/legacy-unowned outcomes through authenticated history/UI.
Do not interpret execution_uncertain=false on a legacy row as proven success or
proven absence of effects. The old APIs must be removed from runtime call sites
or reject token-owned runs before enforcement can be claimed.

No timeout can establish whether an external recipient committed an effect.
Lease loss must not trigger blind replay; scoped readonly observation and
independent reconciliation remain necessary. Business authorization is separate.
