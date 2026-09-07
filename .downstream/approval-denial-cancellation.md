# Atomic denial cancellation candidate

The native denial handler previously committed the signed decision then spawned
an unjournaled task to cancel the run. A process exit could strand a denied run
in waiting_approval. Cancellation also used an earlier read without a gate CAS.

The handler now updates the exact waiting run in the same command transaction.
The database matches community, run, workflow and step against the denied token.
A mismatch rejects the command and rolls back the decision. Trace and snapshot
are untouched; a terminal or newer-gate run cannot be overwritten.

The PostgreSQL regression calls the shared production cancellation function and
tests pending denial, foreign tenant, wrong step, replay, rollback and commit.
This is database seam evidence, not a signed HTTP/UI crash rehearsal. Approval
suspension enablement, durable grant recovery and native history remain separate
unfinished work. No automatic release or production activation is implied.
