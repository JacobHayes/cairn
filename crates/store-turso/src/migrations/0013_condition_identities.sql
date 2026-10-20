-- The identities a node's condition compares an identity or identity-list decision with
-- (E6), beside the answer, role fill and participation rows, so the graphs referring to an
-- identity are found exactly instead of by searching condition text. A text decision's
-- comparison that spells a key names no one. Maintained by every commit that writes nodes;
-- the rows of the graphs that exist are filled by the code step that follows this file.
CREATE TABLE condition_identities (
  graph_id TEXT NOT NULL,
  node TEXT NOT NULL,
  identity TEXT NOT NULL,
  PRIMARY KEY (graph_id, node, identity)
) STRICT;

CREATE INDEX condition_identities_identity ON condition_identities (identity);
