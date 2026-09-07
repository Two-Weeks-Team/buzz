# Step attempt journal prerequisite

Migration 0046 and desired schema add a tenant/run/step-keyed journal with epoch,
step identity, SHA-256 action digest, DB-clock start/return times and a bounded
returned-result object. No execution token or resolved input is stored here.
No historical backfill is attempted. No return means unknown, not no effects.

The begin primitive locks the run, then checks its live lease in a fresh SQL
statement. It requires the exact current step, token and epoch; repeated attempts
are denied even across epochs. At most 4096 step slots exist per run. The return
primitive requires the same live ownership and a previously unreturned intent;
result and optional current-step advancement share one bounded transaction.
Suspension callers must keep the gate index. Lock/statement timeouts are 1s/5s.
An uncertain commit response is not permission to dispatch or retry.

The table participates in community write fencing, desired-state reconciliation
and child-before-parent deletion inventory. Tests exercise real desired and
migration-managed PostgreSQL, duplicate begin concurrency, foreign scope/wrong
owner, skipped future index, stale result denial, bounded payload and second-write
fault rollback. A recorded return is executor evidence, not provider reconciliation.

This patch is only the DB prerequisite: executor dispatch, authenticated reads,
crash trace presentation and effect reconciliation are not connected yet.
Existing runtime behavior and earlier wire evidence must not be represented as
using this journal until that integration is separately verified.
