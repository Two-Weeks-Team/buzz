# Step journal read candidate

`GET /workflows/{workflow_id}/runs/{run_id}/attempts` reads relay-owned DB
evidence, like the existing runs/approvals read models. It does not fabricate
signed lifecycle events; lifecycle publication is separate work. NIP-98 signs
the exact query URL, with existing host tenant, admission, replay, membership
and workflow channel access checks. The run must belong to that workflow.

`limit` defaults to 16 and is at most 32. `after_index` is an ascending exclusive
cursor (0..4095). The envelope contains workflow_id, run_id, attempts and next
(null or an after_index object). The DB query binds all three scope identifiers,
uses the step key and permits one extra lookahead row. Each stored result has
the journal's existing 64KiB DB text limit. No execution token is returned.

Rows expose index, epoch, step ID, action digest, insert time, return time and
recorded result. Null return/result is unconfirmed, not no effects. No rows may
mean a legacy run, never proof of no work. A recorded result is executor evidence,
not independent provider reconciliation. Pages are observations, not a frozen
cross-request snapshot. No read triggers execution or state repair.

`buzz workflows attempts --workflow UUID --run UUID [--limit 16] [--after-index N]`
uses the signed GET client and checks the response scope/page envelope. NIP-11
advertises `buzz.workflow-attempts.v1` separately from workflow history so older
relays can remain explicitly unsupported in clients. Desktop wiring is pending.

DB regressions cover 35-row paging, caps, tenant/workflow mismatch, absent result
and token-free serialization in both schema modes. CLI tests pin path/parameter
validation. Signed live auth, pagination and crash intent persistence are covered
by the private opt-in wire harness and must be run against the newly built revision.
