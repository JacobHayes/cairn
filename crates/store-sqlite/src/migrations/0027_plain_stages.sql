-- A stage is an ordinary group. The code step before this one moved what the group's bounds
-- said into requirements and due-by rules; this rebuilds the nodes table without their
-- columns (a CHECK that names a column blocks dropping it).

CREATE TABLE nodes_new (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  "key" TEXT NOT NULL,
  parent_key TEXT,
  id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('decision', 'deliverable', 'action', 'milestone', 'group')),
  title TEXT NOT NULL,
  description TEXT,
  weight INTEGER CHECK (weight BETWEEN 0 AND 1000),
  relevant_when TEXT CHECK (json_valid (relevant_when)),
  due_by TEXT CHECK (json_valid (due_by)),
  not_before TEXT CHECK (json_valid (not_before)),
  estimate INTEGER CHECK (estimate BETWEEN 0 AND 365),
  placeholder INTEGER CHECK (placeholder IN (0, 1)),
  requires_artifact INTEGER CHECK (requires_artifact IN (0, 1)),
  is_final INTEGER CHECK (is_final IN (0, 1)),
  auto_done INTEGER CHECK (auto_done IN (0, 1)),
  prompt TEXT,
  answer_type TEXT CHECK (answer_type IN (
    'boolean', 'single_choice', 'multi_choice', 'text', 'date', 'identity', 'identity_list'
  )),
  choices TEXT CHECK (json_valid (choices)),
  fills_role TEXT,
  feeds_milestone TEXT,
  help TEXT,
  requires_note INTEGER CHECK (
    requires_note IS NULL OR (requires_note IN (0, 1) AND kind IN ('deliverable', 'action'))
  ),
  checklist TEXT CHECK (
    checklist IS NULL OR (json_valid (checklist) AND kind IN ('deliverable', 'action'))
  ),
  requires_checklist INTEGER CHECK (
    requires_checklist IS NULL
    OR (requires_checklist IN (0, 1) AND kind IN ('deliverable', 'action'))
  ),
  from_route TEXT,
  from_version INTEGER CHECK (from_version >= 1),
  PRIMARY KEY (graph_id, "key"),
  CHECK (kind IN ('deliverable', 'action') OR (estimate IS NULL AND placeholder IS NULL)),
  CHECK (kind = 'deliverable' OR requires_artifact IS NULL),
  CHECK (kind = 'milestone' OR (is_final IS NULL AND auto_done IS NULL)),
  CHECK ((kind = 'decision') = (prompt IS NOT NULL AND answer_type IS NOT NULL)),
  CHECK (kind = 'decision' OR help IS NULL),
  CHECK (choices IS NULL OR answer_type IN ('single_choice', 'multi_choice')),
  CHECK (fills_role IS NULL OR answer_type IN ('identity', 'identity_list')),
  CHECK (feeds_milestone IS NULL OR answer_type = 'date')
) STRICT;

INSERT INTO nodes_new
  SELECT graph_id, "key", parent_key, id, kind, title, description, weight, relevant_when,
         due_by, not_before, estimate, placeholder, requires_artifact, is_final, auto_done,
         prompt, answer_type, choices, fills_role, feeds_milestone, help, requires_note,
         checklist, requires_checklist, from_route, from_version
  FROM nodes;

DROP TABLE nodes;
ALTER TABLE nodes_new RENAME TO nodes;

CREATE INDEX nodes_siblings ON nodes (graph_id, parent_key, id);
