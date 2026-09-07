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

An independent recovery worker now scans untouched Pending runs in keyset pages
of at most 32, after the action sink is wired. It validates the original snapshot,
run/workflow/channel/owner binding, current Active/enabled workflow and current
owner membership (including elevated authority for the original actions). It
then uses the same capacity permit and atomic initial claim as normal dispatch.
Legacy inputs fail closed; edits do not replace original input or definition.
Concurrent recovery/normal dispatch cannot both claim the same run. Invalid rows
are paged past rather than starving later valid rows. Each page awaits its tasks
and sleeps 60 seconds before the next scan, so this is not a 60-second SLA.
Running is never selected or reset; unknown effects still require reconciliation.

PostgreSQL tests cover bounded inventory/exclusion of progress, legacy and scope
rejection, disabled workflows, removed membership, original question/input after
definition edits and two competing recovery calls yielding one approval gate.
Real signed ingress/restart wire proof must be recorded separately per revision.
Schedule-fire claim and run creation still have a separate-commit gap. Post-claim
Running uncertainty, lifecycle outbox and business effect reconciliation remain
unfinished and are not replaced by this snapshot.
