# Granted gate recovery candidate

The relay starts a recovery loop after its action sink is initialized. It scans
at most 32 granted waiting runs per 60-second iteration using a tenant/run keyset
cursor. An empty page resets the cursor. Each page finishes before the next
starts, limiting concurrent recovery tasks. Invalid gates remain durable and the
cursor advances past them; no poison first row can permanently starve later rows.

Both command-triggered and recovered execution use the same snapshot validation,
current owner authority check and atomic granted-gate claim. The scan itself is
host-only inventory, not authority. Running rows are excluded: no blind replay
after a process crash once external effects may have begun.

Database regression binds the production scan: pending rows are omitted,
committed grants are discoverable without notification, cursor is exclusive,
and claimed running rows disappear. This does not prove a restarted relay has
completed an actual workflow. Finalizer suspension, expiry, durable user-visible
history and the post-claim unknown-effect recovery contract remain unfinished.
No production deployment or release is implied.
