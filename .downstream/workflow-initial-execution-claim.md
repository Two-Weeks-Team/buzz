# Initial execution claim candidate

Initial execute_run previously updated any run to Running and cleared its trace.
Repeated dispatch could therefore execute twice or erase suspended/terminal
progress. It now atomically claims only a scoped Pending row at step zero with
an empty trace, preserving original trigger context and existing start time.
Concurrent claim attempts have one winner.

Capacity exhaustion, an unavailable claim and losing the claim return
StartNotClaimed before executing steps. The shared finalizer ignores this
non-owning result rather than overwriting another worker's run. This also leaves
a Pending run pending when capacity is exhausted. It is not automatically retried
by this patch; a durable pending dispatcher remains required.

Actual PostgreSQL regression covers competing claims, tenant mismatch, every
non-Pending state and malformed Pending progress. The native executor/finalizer
PostgreSQL test repeats an initial dispatch after suspension and proves trace,
context, and WaitingApproval survive the losing finalizer.

This prerequisite does not implement Pending restart recovery, capture a pinned
initial definition, solve post-claim Running interruption, add lifecycle outbox,
or authorize business effects. A lost claim response can still leave Running
without execution; it must not be blindly replayed. Those gaps remain explicit.
