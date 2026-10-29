-- There is one kind of route. The insertion records are gone (the code step before this one
-- moved each onto its root as a pointer), a route no longer has a kind, and the event that
-- recorded an insertion is named for the route.

DROP TABLE insertions;

ALTER TABLE routes DROP COLUMN kind;

UPDATE events SET event_type = 'route_inserted' WHERE event_type = 'segment_inserted';
