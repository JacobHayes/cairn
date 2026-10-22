-- A person's place in the tour of Cairn: one row per user, outside every domain. The practice
-- journey has no foreign key: the row outlives the journey, which the tour deletes.

CREATE TABLE tour (
  user_id TEXT PRIMARY KEY REFERENCES users(id),
  state TEXT NOT NULL CHECK (state IN ('started', 'finished', 'dismissed')),
  practice_journey TEXT,
  updated_at INTEGER NOT NULL
) STRICT;
