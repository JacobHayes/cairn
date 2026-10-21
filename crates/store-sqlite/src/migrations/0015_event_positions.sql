-- A log position is never used twice: a hard delete removes a journey's events (A19), and
-- without AUTOINCREMENT SQLite numbers the next event after the largest position left, so
-- the delete's own event could take a removed one and a history cursor past it would skip
-- it. `events` is rebuilt with AUTOINCREMENT; its node rows are set aside meanwhile, since
-- dropping the table they refer to would fail their deferred foreign key.

CREATE TABLE events_new (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  patch_id TEXT NOT NULL,
  ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
  log_kind TEXT NOT NULL CHECK (log_kind IN ('journey', 'route', 'deployment')),
  log_id TEXT,
  event_type TEXT NOT NULL,
  actor_user TEXT NOT NULL,
  agent TEXT,
  confirming_user TEXT,
  subject TEXT NOT NULL CHECK (json_valid(subject)),
  at INTEGER NOT NULL,
  note TEXT,
  delta TEXT NOT NULL CHECK (json_valid(delta)),
  UNIQUE (patch_id, ordinal),
  CHECK ((log_kind = 'deployment') = (log_id IS NULL))
) STRICT;
CREATE TEMP TABLE event_nodes_kept AS SELECT * FROM event_nodes;
DELETE FROM event_nodes;
INSERT INTO events_new SELECT * FROM events;
DROP TABLE events;
ALTER TABLE events_new RENAME TO events;
CREATE INDEX events_log ON events (log_kind, log_id, seq);
CREATE INDEX events_actor ON events (actor_user, seq);
CREATE INDEX events_at ON events (at);
INSERT INTO event_nodes SELECT seq, node FROM event_nodes_kept;
DROP TABLE event_nodes_kept;
