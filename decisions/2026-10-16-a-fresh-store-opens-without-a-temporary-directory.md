# [patina] [turso] A fresh store's migrations keep their temporary structures in memory

Question: the nightly `sim` job's crash sweep (durability testbed, leg 5) has failed since the
segments and barrier-removal migrations landed: crash points past the
open's thirteenth write never fired, and some runs ended `unavailable` at the crash point
instead of restarting. Why, and what changes?

Cause: Turso creates a temporary directory under the operating system's temporary directory
for each `DROP TABLE` and removes it when the statement ends. A fresh store runs two of them
(migration 7 drops the old `node_states`, migration 8 drops `log_barrier`). Patina, at the
pinned revision, cannot crash after the guest removes a directory (already recorded in
`2026-10-07-the-log-sync-fix-nothing-is-answered-from-a-write-until.md`): the injected crash
reaches the guest as `EIO` or never fires. Cairn's product code was not wrong; the sweep had
been reaching its crash points only because no earlier store removed a directory while
opening. Traced by opening the testbed at the commit before the segments migration (crash at
write 35 restarts) and at it (does not), and by watching the directory calls statement by
statement.

Call: `migrate::run` sets `PRAGMA temp_store = MEMORY` on the connection it migrates over, so
the migrations touch no directory but the store's own. A store test opens a fresh database
with `TMPDIR` naming a directory that does not exist and fails without the pragma.

The sweep then reaches every crash point again, and the smoke campaign's "a failed write
retried" oracle, which fired in 6 of 96 generations at 30 per mille before, fired in none
after (the changed operation sequence draws other faults), so the campaign's fault bands go
from 30 to 100 per mille: 4 of 32 generations fire it with the fix, and every generation
still passes. The run pinned for the failed-log-fsync finding moves from seed 2 to seed 5 for the
same reason (seed 2's first fsync fault no longer lands on a commit's); seed 5's first failing
fsync is a commit's and ends the run failed closed.

Alternatives:

- Editing the two migrations: released, and a migration is never edited.
- A baseline schema for fresh stores, skipping the migrations: a second copy of the schema to
  keep equal to the migrated one, for a harness limit.
- `temp_store = MEMORY` on every connection: also removes any later statement's temporary
  files, but lets a large sort grow in memory; the migrations are the only statements known
  to remove a directory, and the sweep fails if another one starts to.
- Fixing patina (a removed directory should not stop crash points): the right place for the
  limit, and a pin bump would let the pragma go. Not done here: patina is another repository.

What would change it: a patina that crashes after a directory removal (drop the pragma, its
test, and the README finding); a Turso that does not use a directory for `DROP TABLE`.
