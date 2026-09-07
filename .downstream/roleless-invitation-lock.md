# Roleless membership writer — candidate

NIP-29 kind 9000 already defines omitted role as preserving an active role,
defaulting to member only for a new or removed membership. Previously the relay
resolved that role before entering the membership writer transaction, turning
an unprivileged-in-intent no-role request into a stale explicit promotion or
demotion when another writer changed the target in between.

The relay now forwards absence through `Db::add_member_preserving_role`.
The shared writer resolves the active role only after acquiring the existing
per-community/per-channel advisory lock. Explicit-role calls retain their
current API and behavior; all existing authority and last-owner checks remain.
This is a contract-preserving race fix, unlike the intentional downstream
private-invitation authority policy. No event kind, HTTP endpoint, schema or
SDK change is needed.

The PostgreSQL regression holds a real writer lock, observes the second writer
waiting in `pg_stat_activity`, promotes the target, commits and asserts the
waiting roleless request preserves the promotion. It also checks new-member,
removed-member reactivation, explicit guest assignment and denied inviter
paths. The test is registered in the focused downstream workflow and uses only
the configured isolated test database.

This does not supply signed business approval, revoke in-flight work, or make
membership changes and event publication one atomic transaction. Consumers
must still re-query results and durably reconcile unknown outcomes. The change
is not qualified for production by a local debug test alone.
