# Approval decision transaction — candidate

The grant/deny command handlers previously kept the signed event in an open
transaction but wrote the decision through the pool. A failed event commit
could leave a resolved approval without its signed command record. Both
handlers now pass the event transaction executor to the existing scoped
decision function. Pending-state CAS and a database wall-clock expiry check
apply to grants/denials at the write boundary.

The real PostgreSQL regression exercises this same decision function through
rollback, commit, replay conflict and expired grant/denial. It proves the DB
primitive, not a rendered approval flow or full relay crash recovery.

Approval gates remain disabled by the existing `approval_not_supported` path.
Do not simply change that status to waiting: safe enablement still needs
atomic suspension/approval persistence, immutable execution definitions,
durable resume claims, owner reauthorization, step/effect recovery and emitted
history. Denial cancellation still uses a post-commit asynchronous task.
The generic workflow approval identity must not independently authorize an
operator's business side effects; those must stay behind the configured
authority boundary. This patch creates no new approval engine or public API.
