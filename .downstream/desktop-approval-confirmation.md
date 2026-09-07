# Desktop approval confirmation candidate

The history card now exposes review then explicit confirm for an exact pending,
unexpired gate in the current waiting run. The live identity query supplies the
signer (not the community's display-only stored pubkey); the active community
supplies relay scope. Unsupported approver specifications fail closed, matching
the relay's exact-key/any contract. The UI is not the authorization boundary.

Confirmation repeats the persisted question, workflow, run, step, reference,
relay and signer, accepts an optional note, and explains that approving a gate
does not establish execution completion or separate business authority.
Missing question/scope, invalid expiry, another approver or a terminal run do not
expose confirmation. Changing gate or actor scope remounts the confirmation card.
Both card and panel use immediate refs to fence concurrent clicks while the
shared native mutation is pending. Native scoped preflight remains authoritative.

A write error produces an unconfirmed outcome, not inferred failure/success and
not an automatic retry. Refresh reads history. A terminal decision replaces the
pending card; a new explicit review is required for any subsequent attempt.

Unit/SSR regressions cover eligibility and no mutation at render. Browser tests
use the actual E2E bundle with synthetic native responses, exercise explicit
confirmation, rapid double click, denial, unconfirmed response and refresh, and
capture visible artifacts in the private operations repository. These are not
native webview dispatch, real-founder or production approval evidence. Live
native command/relay evidence is separately recorded for the transport patch.
