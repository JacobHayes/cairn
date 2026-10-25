-- Finished work is stale when something it relied on is reopened after it finished. A day
-- cannot say which of two writes on the same day came first, so each finish and each reopen
-- now records the journey revision it was written at, which orders them exactly. The code
-- step after this fills them from the log, and the step after it drops the reopen's days,
-- which nothing reads any more.

ALTER TABLE node_states ADD COLUMN finished_revision INTEGER CHECK (finished_revision >= 1);

ALTER TABLE node_states ADD COLUMN reopened_revision INTEGER CHECK (reopened_revision >= 1);

ALTER TABLE node_states ADD COLUMN reopened_finished_revision INTEGER
  CHECK (reopened_finished_revision >= 1);
