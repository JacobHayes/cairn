-- Order is no longer a guard; the code step before this one removed the order guards from
-- every stored bypass. Reopening a finish now leaves a record on the node it reopens, kept
-- while the node is open again: the day, and the finish it undid. Finished work that relied
-- on the node reads it to go stale.

ALTER TABLE node_states ADD COLUMN reopened_on TEXT;

ALTER TABLE node_states ADD COLUMN reopened_finished_on TEXT
  CHECK ((reopened_finished_on IS NULL) = (reopened_on IS NULL));
