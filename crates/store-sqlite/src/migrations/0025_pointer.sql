-- A journey keeps only the route version it was made from. The code step before this one
-- rewrote the stored JSON; these statements drop what held the rest: each node's provenance,
-- the local-edit markers and the tombstones. The events that only recorded a provenance or a
-- marker go, and an upgrade or a re-link reads as a journey edit.

ALTER TABLE node_states DROP COLUMN provenance;

DROP TABLE local_edits;

DROP TABLE tombstones;

DELETE FROM event_nodes WHERE seq IN (
  SELECT seq FROM events WHERE event_type IN ('provenance_changed', 'local_edit_changed')
);

DELETE FROM events WHERE event_type IN ('provenance_changed', 'local_edit_changed');

UPDATE events SET event_type = 'journey_edited'
  WHERE event_type IN ('journey_upgraded', 'relinked');
