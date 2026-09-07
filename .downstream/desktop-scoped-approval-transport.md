# Scoped native approval transport candidate

Desktop previously sent a `t` tag, whereas the relay consumes the stored hash
in `d` (or `e`). It returned only event_id to a frontend expecting an unrelated
token/status/run/workflow envelope. Both mismatches are removed.

Decision requests require workflow/run UUIDs, an exact 64-hex approval reference,
and the UI-captured relay URL and signer public key. Native code captures and
asserts relay and keys before any await; the authenticated preflight read and
signed submission use those same values. The preflight requires exactly one
matching, pending, unexpired approval in the scoped run response. The relay still
enforces designated approver, expiry at write time and racing decisions.

The acknowledgement must name the requested run and decision. Event acceptance,
a duplicate event message, missing fields and run completion are not decision
acknowledgements. The TypeScript adapter also rejects an old or mismatched wire
shape. The mutation never automatically retries and refreshes history on either
success or error: transport failure can follow a committed decision.

Native unit tests cover signed event shape, binding and acknowledgements. The
explicitly ignored desktop_approval_loopback_probe additionally exercises the
actual command implementation against a provisioned synthetic loopback relay;
its fixed test keys cannot be changed to a real user key via the fixture.
It is not run by ordinary unit CI. Native test builds omit bundled sidecars only
through TAURI_CONFIG and build bundled Opus to avoid host-architecture mismatch.
The actual product packaging configuration and dependency locks remain unchanged.

The fork's native unit CI uses the standard public-repository macos-26 runner,
not a larger/paid runner or a billing settings change. GitHub documents this
standard public runner class as free:
https://docs.github.com/en/actions/reference/runners/github-hosted-runners

The pending Desktop card is still read-only until its scoped confirmation flow
is connected and rehearsed. These native transport changes do not grant business
spending, organization provisioning, or customer-send authority.
