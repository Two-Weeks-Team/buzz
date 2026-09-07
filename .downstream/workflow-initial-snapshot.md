# Initial dispatch snapshot candidate

Event, schedule, manual and webhook entrypoints now store the exact validated
definition, original typed trigger and workflow/channel/owner bindings when they
create a Pending run. Serialization failure is not converted to missing input.
The initial wrapper uses buzz_execution_version=2 and an initial field; it is
deliberately distinct from approval snapshot v1, whose gate is mandatory.
Strict decoding rejects legacy/default input and unsupported wrapper versions.

The generic create_workflow_run database function can participate in the caller's
transaction. Manual signed commands use that path so the event and run/snapshot
commit together, rather than exposing a run before its command commits.
PostgreSQL tests prove precommit invisibility and rollback/commit behavior. The
event-path authority regression also decodes the original snapshot from its run.

These are dispatch-recovery prerequisites, not a completed recovery mechanism.
The Pending recovery scan still needs to validate stored bindings and current
owner authority before the existing atomic initial execution claim. Legacy
Pending rows must not be recovered from the latest definition or empty context.
Schedule-fire claim and run creation still have a separate-commit gap. Post-claim
Running uncertainty, lifecycle outbox and business effect reconciliation remain
unfinished and are not replaced by this snapshot.
