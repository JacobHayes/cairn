-- A group marked done keeps the reason it was given, as a skipped node does, so the column
-- that held a skip's reason holds either. The code step before this one renamed the key in the
-- JSON columns; this statement renames the column. The state CHECK already lists `done`.

ALTER TABLE node_states RENAME COLUMN skip_reason TO reason;
