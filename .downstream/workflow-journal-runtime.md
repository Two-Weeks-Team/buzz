# Durable step journal runtime candidate

Initial and resumed sequential executors now commit an exact owned step intent
before dispatch. Its digest is SHA-256 of the serialized resolved action. A
condition-false step instead hashes an explicit condition-false marker, records
skipped, and advances without dispatch. Definitions are bounded to 4096 steps,
matching the durable journal capacity before any execution can start.

Completed/skipped returns are persisted with progress before the next step.
Suspension records only `suspension_requested`, no approval token, and does not
advance. Actual gate/snapshot suspension still belongs to the atomic finalizer.
An unconfirmed journal write yields execution_journal_unconfirmed and the owned
finalizer conservatively abandons the run as uncertain. No automatic replay is
introduced. Timeout/dispatch errors can leave intent without return; this is
unknown outcome evidence, not proof of no effect.

Native executor tests bind real PostgreSQL to condition skip → delay completion
→ gate suspension → approved resume with a new epoch and to lease loss during a delay.
The resume fixture uses the DB approval primitive, not signed human transport.
A preexisting ambiguous
intent blocks a 30-second delay before dispatch, bounded by a two-second test
deadline. These are isolated executor/finalizer tests, not provider or process
crash proofs. The API/CLI/Desktop journal read surface and real crash/response-loss
wire rehearsal remain separate work. Earlier wire results predate this patch.
