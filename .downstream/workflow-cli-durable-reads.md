# Durable CLI workflow reads

`workflows runs --workflow UUID [--limit 1..100]` now reads the relay's signed
structured run endpoint instead of unpublished lifecycle events. It returns the
`{runs,next}` envelope, deliberately replacing the previous empty event-array
projection. `next.before` and `next.before_id` can be passed as `--before` and
`--before-id`; both are required together. Timestamp precision and offset survive
URL encoding. This is a database read, not an event read.

`workflows approvals --workflow UUID --run UUID` returns `{approvals}` with stored
references and decisions. Use the returned approval_ref with `workflows approve
--approval-ref HEX`. Reads use the existing host/tenant-scoped NIP-98 client.
Malformed JSON or missing array envelopes are errors, never empty success.

Production helper tests cover URL encoding, pagination arguments, malformed
responses and existing reference normalization. Actual relay qualification must
also exercise pagination and approval readback; mocks alone are insufficient.
This patch does not emit native lifecycle events or prove rendered client UX.
