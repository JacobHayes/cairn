-- A deliverable's or action's checklist (items of an id and a title, as JSON, null when it has
-- none) and the flag that makes every item ticked a guard on done, and a journey's ticks:
-- one row per ticked item, apart from the node's state so that ticking two items, or ticking
-- and a transition, are different rows.

ALTER TABLE nodes ADD COLUMN checklist TEXT
  CHECK (checklist IS NULL OR (json_valid(checklist) AND kind IN ('deliverable', 'action')));

ALTER TABLE nodes ADD COLUMN requires_checklist INTEGER
  CHECK (requires_checklist IS NULL
    OR (requires_checklist IN (0, 1) AND kind IN ('deliverable', 'action')));

CREATE TABLE ticks (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  node TEXT NOT NULL,
  item TEXT NOT NULL,
  PRIMARY KEY (graph_id, node, item)
) STRICT;
