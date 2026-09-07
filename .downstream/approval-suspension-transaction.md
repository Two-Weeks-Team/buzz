# Atomic suspension persistence prerequisite

`Db::suspend_workflow_run` persists trace, current step, a versioned execution
context wrapper and the hashed-token approval request in one transaction.
It predicates the run update on community, workflow identity, running status,
non-regressing step and a future expiry. A failed approval insert rolls back
the status/context/trace update. A duplicate finalizer cannot overwrite a gate
that is already waiting.

The PostgreSQL regression covers not-running and expired gates, token collision
rollback, successful persistence, duplicate finalization and a foreign tenant.
It uses a structural context fixture, not a valid executable workflow.

This is a DB seam only. The executor/finalizer does not call it yet and approval
gates remain unsupported. The execution-context JSON's semantic validation,
definition stability across multiple gates and owner identity binding must be
implemented in the engine before wiring it in. The current legacy resume
parser must not consume this wrapper: it currently falls back to an empty
trigger. A typed/version-checked decoder, durable resume claims, recovery and
history emission are required together for safe enablement.

No database column/migration is added. The wrapper uses the existing JSONB
trigger_context field; old rows remain distinguishable and must fail closed
for new snapshot-dependent resume logic rather than being silently upgraded.
