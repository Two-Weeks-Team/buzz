# Native suspension candidate

The shared finalizer now validates the executor's typed snapshot and stores the
gate with the run, prior trace and immutable execution context transactionally.
The trace contains the resolved question, approver specification and hashed
approval reference; plaintext tokens are not exposed. Existing run/approval reads
can return this persisted data. This replaces approval_not_supported failure.

Missing/mismatched snapshots and overflowing expiry calculations fail closed.
Storage failures do not overwrite another gate or claim completion. The existing
running record remains for investigation, not automatic external-effect replay.
This is not yet sufficient failure UX or post-claim recovery.

The PostgreSQL test executes the actual request_approval action, calls the shared
finalizer and reads the resulting waiting run, typed context and approval row.
No HTTP signature path, user-facing rendered screen, expiry reaper or restart
end-to-end completion is claimed by this test. Native lifecycle events and
post-claim unknown-effect handling remain unfinished. Candidate only; no release.
