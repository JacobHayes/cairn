-- A21, B13: a route is a process or a segment, and a graph holds the insertions of segment
-- versions. Rows written before this are processes with no insertions, so they load as
-- they did.

ALTER TABLE routes
  ADD COLUMN kind TEXT NOT NULL DEFAULT 'process' CHECK (kind IN ('process', 'segment'));

-- One placement of a segment version in a graph. The maps and the members are JSON the
-- schema validates before it writes, as condition trees are.
CREATE TABLE insertions (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  key TEXT NOT NULL,
  segment TEXT NOT NULL,
  version INTEGER NOT NULL CHECK (version >= 1),
  parent_key TEXT,
  roles TEXT NOT NULL CHECK (json_valid(roles)),
  kinds TEXT NOT NULL CHECK (json_valid(kinds)),
  nodes TEXT NOT NULL CHECK (json_valid(nodes)),
  PRIMARY KEY (graph_id, key)
) STRICT;

CREATE INDEX insertions_by_segment ON insertions (segment, version);

-- A node copied in from a segment has its own provenance. A CHECK cannot be altered, so the
-- table is rebuilt with the wider one and its rows carried over.
CREATE TABLE node_states_new (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  node TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN (
    'todo', 'active', 'done', 'skipped', 'open', 'decided', 'pending', 'reached', 'derived'
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
