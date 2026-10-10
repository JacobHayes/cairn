-- The log barrier (migration 4) is gone: a commit the store cannot settle now fails the store
-- closed instead (`durable.rs`), so nothing writes this table. IF EXISTS because a database
-- opened again after its migrations were rolled back to an earlier version (a test) has lost it.

DROP TABLE IF EXISTS log_barrier;
