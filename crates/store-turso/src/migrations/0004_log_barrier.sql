-- The barrier (`durable.rs`): one row the store rewrites to sync the logical log past a
-- commit whose own log sync failed, and at every open. Nothing else writes it, so a
-- barrier conflicts with no commit.

CREATE TABLE log_barrier (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  count INTEGER NOT NULL CHECK (count >= 0)
) STRICT;

INSERT INTO log_barrier (id, count) VALUES (1, 0);
