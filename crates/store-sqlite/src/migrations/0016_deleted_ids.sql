-- A hard delete is the deployment's, and a deleted route's id is never reused either: one
-- table of deleted ids for journeys and routes replaces the deleted journeys'. The code step
-- before this one rewrote the JSON that names a deleted journey. The journeys following a
-- route are found by their lineage, for a route's detail and for whether a route is in use.

CREATE TABLE deleted_ids (
  kind TEXT NOT NULL CHECK (kind IN ('journey', 'route')),
  id TEXT NOT NULL,
  deleted_at INTEGER NOT NULL,
  PRIMARY KEY (kind, id)
) STRICT;

INSERT INTO deleted_ids (kind, id, deleted_at)
  SELECT 'journey', id, deleted_at FROM deleted_journeys;

DROP TABLE deleted_journeys;

CREATE INDEX journeys_by_lineage ON journeys (lineage_route, lineage_version);
