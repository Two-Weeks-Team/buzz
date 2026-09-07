# Approval wire entrypoint correction

Isolated signed manual-trigger testing found that initial manual and webhook
execution called execute_from_step(0). The downstream resume claim correctly
rejected these unapproved new runs, but that made the initial entrypoints fail.
They now call execute_run; only approval resume uses execute_from_step.

Structured approval reads return a 32-byte hex reference, but the CLI only
accepted a raw UUID token. The CLI now accepts --approval-ref as an alias for
--token and preserves valid hex references instead of hashing them again.
Legacy UUID input is still hashed exactly as before. Invalid input is rejected.

CLI normalization regression covers legacy UUID, upper/lowercase reference and
malformed input. Signed HTTP local probe confirms reference-based grants and
designated-approver rejection. Webhook invocation, full lifecycle events, expiry,
restart and visible client workflow remain separate proof requirements. Candidate
only; no production deployment is implied.
