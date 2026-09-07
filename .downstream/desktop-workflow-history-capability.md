# Desktop workflow history capability candidate

The existing history trigger was commented out pending an advertised capability.
This fork now advertises `buzz.workflow-history.v1` in its NIP-11 document for its
structured run and approval read endpoints. This does not grant access: signed
queries retain channel/tenant authorization and existing server-side checks.

The Desktop dialog probes the connected relay while open, with a five-second
abort bound. Only the exact advertised version exposes the history trigger and
popover. Unknown versions, malformed documents and unavailable relays remain
hidden. The probe has no cross-community global cache and aborts on unmount/close.
Relay CORS restrictions are preserved, not relaxed to expose this UI.

The browser rehearsal uses the actual E2E Desktop bundle and production UI with
synthetic IPC run/approval responses and intercepted NIP-11 capability data.
It is a rendered UI check, not connected native IPC, real relay CORS, founder
approval or production deployment evidence. Desktop decision actions and durable
notifications remain unfinished. Exact-version predicate and relay advertisement
regressions run in the fork's scoped CI jobs.
