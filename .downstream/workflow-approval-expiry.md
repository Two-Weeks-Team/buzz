# Bounded approval expiry

The recovery loop now expires up to 32 overdue pending gates per iteration.
Database wall-clock time determines eligibility. Approval rows are locked first,
matching signed grant/deny ordering; concurrent workers skip locked approvals.
Run cancellation and approval expiration commit together, matching tenant, run,
workflow and current step. Trace/context are preserved. Granted decisions, future
deadlines and runs at another gate are not overwritten.

The transaction uses local 1-second lock and 5-second statement timeouts. Approval
SKIP LOCKED alone cannot protect the subsequent run-row lock. These settings are
transaction-scoped and do not modify other pooled sessions. The regression holds
the run lock and requires an error within 3 seconds, then injects a failure in the
approval write after run cancellation and verifies both records roll back before
normal expiry can succeed. Test-only fault DDL is scoped to a generated run UUID.

Expiry failure leaves durable pending state for later retry. This shares the
recovery loop, so 60 seconds is an inter-iteration delay, not a guaranteed expiry
notification SLA while recovered executions are running. Late grant/deny writes
remain rejected at the database deadline regardless of polling delay.

PostgreSQL regression covers a held decision lock, concurrent expiry workers,
expired/future/granted/wrong-step fixtures, preserved trace and late grant denial.
It does not yet prove expiry through a running relay or rendered notification.
Native lifecycle events and post-claim unknown-effect recovery remain separate.
