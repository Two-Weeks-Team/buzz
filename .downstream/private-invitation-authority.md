# Private invitation authority — candidate

This fork deliberately reserves third-party additions to **private** channels
to active channel owners/admins. Relay-wide admin status alone is not enough.
Open-channel behavior is unchanged. Existing active self-target calls retain
their previous idempotent semantics. The target agent's channel-add policy
still applies; this patch does not bypass it.

The pre-storage NIP-29 validator returns a structured denial before accepting
an unauthorized event. The database writer checks the inviter again under the
existing membership advisory lock, including direct callers and reactivation
of removed members. A previously authorized inviter cannot use a stale role
after demotion to extend private membership through that writer.

Compatibility: member-initiated bot additions in private channels will also be
denied. A channel admin must perform those additions. This is an intentional
downstream divergence from the current upstream member-invitation policy,
not a configurable per-channel setting in this first patch. It neither revokes
existing memberships nor deletes previously downloaded history.

## Evidence and remaining gates

- Rust 1.95.0, macOS arm64: seven `channel_authz` tests passed.
- PostgreSQL 17, fresh isolated desired-state schema: the production writer
  regression passed for members, bots, guests, self-target calls, owner/admin
  invitations, removed-member reactivation, inviter demotion and open channels.
- PostgreSQL test discovery convention check passed; the new test is in an
  out-of-line module ending in `postgres_tests` and uses the standard test URL.
- Focused GitHub CI is provided in `downstream-invitation.yml` without publishing
  or deployment permissions. Metadata CI alone is not an authorization test.

Still required before qualification: full relevant CI, signed HTTP/WebSocket
and CLI tests against the newly built relay, client compatibility, migration
schema parity, concurrent demotion/invitation behavior, and deployment-platform
evidence. No schema migration is introduced by this patch. Never interpret a
candidate branch or unit-test success as production approval.
