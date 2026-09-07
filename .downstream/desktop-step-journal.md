# Desktop step journal (candidate)

Run history reads 16 attempts per page only when the relay advertises
`buzz.workflow-attempts.v1`. The native command validates UUIDs and cursor,
captures the expected relay and signer before awaiting, and checks response
scope. The strict client rejects malformed, out-of-order, foreign and unsafe
pages. Reads have no execution or retry side effects.

The view is keyed by relay/signer/workflow/run, ignores late unmounted responses,
and distinguishes absent capability, failed read, empty legacy page, recorded
return and unconfirmed return. Neither a recorded return nor an empty journal
proves external effects. Refresh resets paging, never replays execution.

Evidence: selected frontend tests 13/13; native request bounds 1/1 and existing
approval decisions 3/3; TypeScript and E2E build passed. Headed mock rehearsal
verified 16+1 paging, expanded digest, unconfirmed return, read failure, foreign
scope rejection, empty legacy explanation and recovery by refresh. This is not
a native GUI or deployed-user rehearsal. Signed relay/CLI crash-wire evidence
121/121 belongs to backend revision c8473bb9, not proof of this new native read
command's real relay integration.

The native `workflow_attempts_command_signed_http_preserves_scope_and_errors`
test now invokes the actual Tauri command against an ephemeral loopback HTTP
peer: verifies the Schnorr event, signer, kind 27235, GET and exact URL including
cursor; proves wrong relay/signer are rejected before connection; switches the
active relay/signer before returning the response; and checks foreign scope,
401 and malformed JSON remain errors. This is real native HTTP transport with
synthetic peer responses, not a live relay/database or native GUI rehearsal.
