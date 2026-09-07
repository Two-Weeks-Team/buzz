# Desktop approval decision trace candidate

The raw trace adapter now preserves the persisted question and approval_ref.
WorkflowRunTrace matches decisions by workflow, run, step and exact reference.
Historical waiting_approval without a matching decision displays a request with
current decision unavailable, not an inferred pending or completed gate. Matching
decisions display granted/denied/expired, the note and deciding public key.
Questions and notes are rendered as escaped React text.

Regression tests bind the production selector and actual component server render:
foreign references/scopes are not attached, a missing read is not a decision,
resolved question and note are visible, and granted gates have no stale pending
badge/card. TypeScript and selected Biome checks are separate gates.

Server-rendered markup is not browser-visible or connected Desktop proof. Actual
Desktop rehearsal, native lifecycle events and Desktop approval actions remain
unfinished; the existing read-only approval card is not claimed actionable.
