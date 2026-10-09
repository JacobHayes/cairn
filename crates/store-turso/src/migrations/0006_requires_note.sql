-- G4: a deliverable or action may require a note of its own before `done`. Null on the
-- other kinds, as the other kind-restricted flags are. Rows written before this keep null,
-- which reads as the flag off.

ALTER TABLE nodes
  ADD COLUMN requires_note INTEGER
    CHECK (requires_note IS NULL
      OR (requires_note IN (0, 1) AND kind IN ('deliverable', 'action')));
