# CLI query response contract

The shared `query_multi` boundary now validates that a successful HTTP response
is a JSON array of objects before returning the original bytes. `query` and
paginated queries use this same path. Malformed JSON, wrong top-level shapes,
and non-object rows become `CliError::Other` (exit 4, non-retryable) without
echoing response data. A real empty array remains successful.

This advances the agent-first CLI contract: unavailable data must not become
an empty inbox, roster, search, or fabricated event. Validation is structural,
not complete event-schema, signature, permission, or completeness verification.
Valid object extensions and whitespace are preserved. No relay policy changes.

Loopback HTTP tests exercise the shared client plus actual CLI dispatch for
messages get/search, channels list, users presence and feed get. Separate local
process probes compare stdout/stderr/exit code before and after rebuilding.
The old executable returned exit 0 for malformed inputs and fabricated a blank
event for `[null]`; the fixed executable must fail with no stdout.

Full CLI testing also caught stale workflow command inventory expectations
(`attempts` was missing) and a test-module ordering lint. Both are corrected.
Downstream CI now runs the complete CLI library suite instead of only approval
reference tests. Updated Linux image and production deployment are separate
qualification gates, not implied by these local tests.
