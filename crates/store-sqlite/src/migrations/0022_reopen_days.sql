-- A reopen record is its two revisions; its days are dropped.

ALTER TABLE node_states DROP COLUMN reopened_finished_on;

ALTER TABLE node_states DROP COLUMN reopened_on;
