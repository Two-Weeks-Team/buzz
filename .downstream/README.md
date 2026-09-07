# Downstream maintenance contract

This is a maintained fork of [block/buzz](https://github.com/block/buzz), not an
upstream release. Apache-2.0 licensing and upstream attribution remain unchanged.
Application-specific company policies, credentials, customer data and deployment
configuration do not belong in this public repository.

## Branches and provenance

- `main`: upstream mirror; no downstream commits. Update only by fast-forward
  after inspecting upstream changes. Never force-push a shared branch.
- `agentbase/main`: reviewed downstream integration, based on the exact upstream
  commit in `manifest.json`. No deployment is triggered by this branch.
- `agentbase/<topic>`: one bounded patch with its regression tests and patch
  ledger entry. Merge only after the relevant gates pass.
- Use immutable source SHAs and image digests for deployment. A moving branch
  name or a passing manifest check is not a release qualification.

Local remotes are `origin` (this fork) and `upstream` (block/buzz). Configure
`remote.pushDefault=origin` and set upstream's push URL to `DISABLED` to prevent
accidental upstream writes. These are local settings, not repository protection.

## Upstream updates

1. Activate Hermit, ensure a clean checkout, and fetch upstream. Do not run setup
   or reset against an existing developer/production database.
2. Create `agentbase/sync-<date>` from `agentbase/main`. Review upstream release
   notes, schema changes and each entry in the patch ledger.
3. Merge the selected upstream SHA with DCO sign-off. Resolve conflicts here,
   not on the protected integration branch. Never auto-resolve authorization or
   migration conflicts. Update `upstream_base` to the reviewed SHA.
4. Re-run each patch's regression test, `just ci`, and relay/DB integration tests
   when those components change. Prove the live affected workflow separately.
5. Remove a downstream patch only when the upstream replacement is verified by
   the same regression. Record the upstream PR and migration implications.
6. Merge the reviewed sync branch. Only then fast-forward the mirror `main` to
   the reviewed upstream commit. A sync is not a production rollout.

## Required patch ledger fields

Each `patches` entry has `id`, `summary`, `paths`, `tests`, `upstream_reference`,
and `status` (`candidate` or `qualified`). Tests must exercise the production
seam; a text assertion against source is not sufficient. `qualified` requires
the relevant local, integration and live evidence, not only metadata CI.

The lightweight `Downstream metadata` workflow checks provenance/ledger
structure only. It intentionally does not build or publish an image, request
secrets, or attest that code is production-ready. Upstream workflows are kept
intact; release workflows must not be enabled on the fork until their package
namespace, permissions, signing and explicit promotion gates are reviewed.

## Initial candidate: private-channel invitation authority

Upstream's current VISION explicitly permits member invitations. This fork's
intended policy is stricter: third-party membership changes in private channels
require an active channel owner/admin. This is a documented downstream policy
divergence, not a claim that upstream violates its own contract.

Related upstream history: [PR 4612](https://github.com/block/buzz/pull/4612).
Guest invite work [PR 3673](https://github.com/block/buzz/pull/3673) has a different
scope and must be reviewed on sync. Do not assume an old merged PR describes
current behavior.

The patch must cover both `channel_authz::decide_put_user` and the transactional
`buzz-db` membership writer under its existing advisory lock. Cover ordinary
members/bots/guests, removed-member reactivation, idempotent self-target calls,
authorized owner/admin additions, and unchanged open-channel semantics. Raw
CLI and signed HTTP/WebSocket calls must hit the same server authority.
