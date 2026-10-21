-- A finished milestone is done, like every other finished node. The code step before this one
-- rewrote the JSON columns; these statements move the tables: the state the CHECK lists, the
-- column and the local-edit target that carry the flag's name, and the event type the finishing
-- of a milestone used to have. A table whose CHECK lists the old state is rebuilt (a CHECK
-- cannot be altered).

UPDATE node_states SET state = 'done' WHERE state = 'reached';

CREATE TABLE node_states_new (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  node TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN (
    'todo', 'active', 'done', 'skipped', 'open', 'decided', 'pending', 'derived'
  )),
  provenance TEXT NOT NULL
    CHECK (provenance IN ('from_route', 'from_segment', 'local', 'orphaned')),
  atomic INTEGER NOT NULL CHECK (atomic IN (0, 1)),
  started_on TEXT,
  finished_on TEXT,
  skip_reason TEXT,
  PRIMARY KEY (graph_id, node)
) STRICT;

INSERT INTO node_states_new
  SELECT graph_id, node, state, provenance, atomic, started_on, finished_on, skip_reason
  FROM node_states;

DROP TABLE node_states;
ALTER TABLE node_states_new RENAME TO node_states;

ALTER TABLE nodes RENAME COLUMN auto_reach TO auto_done;

UPDATE local_edits SET target = 'auto_done' WHERE aspect = 'field' AND target = 'auto_reach';

UPDATE events SET event_type = 'node_transitioned' WHERE event_type = 'milestone_reached';
