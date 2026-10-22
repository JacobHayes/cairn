-- A journey's owners (zero or more identities beside its name), by identity, so the journeys
-- referring to an identity are found by owning too.

CREATE TABLE journey_owners (
  journey_id TEXT NOT NULL REFERENCES journeys (id) DEFERRABLE INITIALLY DEFERRED,
  identity TEXT NOT NULL,
  PRIMARY KEY (journey_id, identity)
) STRICT;

CREATE INDEX journey_owners_identity ON journey_owners (identity);
