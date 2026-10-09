-- B2: the rationale an answer was given with: markdown, null when the answer gave no
-- reason. It lives and dies with its answer's row, so a revision that gives none replaces
-- it with null and a reopened decision loses both. Rows written before this keep null.

ALTER TABLE answers
  ADD COLUMN rationale TEXT CHECK (rationale IS NULL OR trim(rationale) <> '');
