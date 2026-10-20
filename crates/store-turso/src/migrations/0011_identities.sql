-- The people and teams journeys refer to are identities (PRD, glossary), under opaque keys.
-- The code step before this one rewrote the JSON columns and filled `identity_key_map`;
-- these statements move the tables, their keys, their column and event names, and the
-- answer types the CHECKs list. A table whose CHECK lists the old answer types is rebuilt
-- (a CHECK cannot be altered). A key the map lacks makes a NOT NULL fail the whole
-- migration, which rolls back.

CREATE TABLE identities (
  key TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK (length(name) > 0)
) STRICT;

INSERT INTO identities (key, name)
  SELECT (SELECT new FROM identity_key_map WHERE old = entities.key), name FROM entities;

CREATE TABLE identity_emails (
  identity TEXT NOT NULL REFERENCES identities (key) DEFERRABLE INITIALLY DEFERRED,
  email TEXT NOT NULL,
  PRIMARY KEY (identity, email)
) STRICT;

INSERT INTO identity_emails (identity, email)
  SELECT (SELECT new FROM identity_key_map WHERE old = entity), email FROM entity_emails;

CREATE INDEX identity_emails_email ON identity_emails (email);

CREATE TABLE identity_aliases (
  alias TEXT PRIMARY KEY,
  identity TEXT NOT NULL REFERENCES identities (key) DEFERRABLE INITIALLY DEFERRED
) STRICT;

INSERT INTO identity_aliases (alias, identity)
  SELECT (SELECT new FROM identity_key_map WHERE old = alias),
         (SELECT new FROM identity_key_map WHERE old = entity)
  FROM entity_aliases;

DROP TABLE entity_emails;
DROP TABLE entity_aliases;
DROP TABLE entities;

-- The references journeys and routes hold, by identity (E6).
CREATE TABLE role_fill_identities (
  graph_id TEXT NOT NULL,
  role TEXT NOT NULL,
  identity TEXT NOT NULL,
  PRIMARY KEY (graph_id, role, identity),
  FOREIGN KEY (graph_id, role) REFERENCES role_fills (graph_id, role)
    DEFERRABLE INITIALLY DEFERRED
) STRICT;

INSERT INTO role_fill_identities (graph_id, role, identity)
  SELECT graph_id, role, (SELECT new FROM identity_key_map WHERE old = entity)
  FROM role_fill_entities;

DROP TABLE role_fill_entities;
CREATE INDEX role_fill_identities_identity ON role_fill_identities (identity);

CREATE TABLE participation_identities (
  graph_id TEXT NOT NULL,
  node TEXT NOT NULL,
  kind TEXT NOT NULL,
  identity TEXT NOT NULL,
  PRIMARY KEY (graph_id, node, kind, identity),
  FOREIGN KEY (graph_id, node, kind) REFERENCES participations (graph_id, node, kind)
    DEFERRABLE INITIALLY DEFERRED
) STRICT;

INSERT INTO participation_identities (graph_id, node, kind, identity)
  SELECT graph_id, node, kind, (SELECT new FROM identity_key_map WHERE old = entity)
  FROM participation_entities;

DROP TABLE participation_entities;
CREATE INDEX participation_identities_identity ON participation_identities (identity);

-- The answers' references go aside while `answers` is rebuilt, since they refer to it.
CREATE TABLE answer_identities_aside (
  graph_id TEXT NOT NULL,
  decision TEXT NOT NULL,
  identity TEXT NOT NULL
) STRICT;

INSERT INTO answer_identities_aside (graph_id, decision, identity)
  SELECT graph_id, decision, (SELECT new FROM identity_key_map WHERE old = entity)
  FROM answer_entities;

DROP TABLE answer_entities;

CREATE TABLE answers_new (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  decision TEXT NOT NULL,
  answer_type TEXT NOT NULL CHECK (answer_type IN (
    'boolean', 'single_choice', 'multi_choice', 'text', 'date', 'identity', 'identity_list'
  )),
  value TEXT NOT NULL CHECK (json_valid(value)),
  rationale TEXT CHECK (rationale IS NULL OR trim(rationale) <> ''),
  PRIMARY KEY (graph_id, decision)
) STRICT;

INSERT INTO answers_new (graph_id, decision, answer_type, value, rationale)
  SELECT graph_id, decision,
         CASE answer_type WHEN 'entity' THEN 'identity' WHEN 'entity_list' THEN 'identity_list'
           ELSE answer_type END,
         value, rationale
  FROM answers;

DROP TABLE answers;
ALTER TABLE answers_new RENAME TO answers;

CREATE TABLE answer_identities (
  graph_id TEXT NOT NULL,
  decision TEXT NOT NULL,
  identity TEXT NOT NULL,
  PRIMARY KEY (graph_id, decision, identity),
  FOREIGN KEY (graph_id, decision) REFERENCES answers (graph_id, decision)
    DEFERRABLE INITIALLY DEFERRED
) STRICT;

INSERT INTO answer_identities (graph_id, decision, identity)
  SELECT graph_id, decision, identity FROM answer_identities_aside;

DROP TABLE answer_identities_aside;
CREATE INDEX answer_identities_identity ON answer_identities (identity);

CREATE TABLE nodes_new (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  key TEXT NOT NULL,
  parent_key TEXT,
  id TEXT NOT NULL,
  kind TEXT NOT NULL
    CHECK (kind IN ('decision', 'deliverable', 'action', 'milestone', 'group')),
  title TEXT NOT NULL,
  description TEXT,
  weight INTEGER CHECK (weight BETWEEN 0 AND 1000),
  relevant_when TEXT CHECK (json_valid(relevant_when)),
  due_by TEXT CHECK (json_valid(due_by)),
  not_before TEXT CHECK (json_valid(not_before)),
  estimate INTEGER CHECK (estimate BETWEEN 0 AND 365),
  placeholder INTEGER CHECK (placeholder IN (0, 1)),
  requires_artifact INTEGER CHECK (requires_artifact IN (0, 1)),
  is_final INTEGER CHECK (is_final IN (0, 1)),
  auto_reach INTEGER CHECK (auto_reach IN (0, 1)),
  opens_at TEXT,
  closes_at TEXT,
  gates INTEGER CHECK (gates IN (0, 1)),
  closes INTEGER CHECK (closes IN (0, 1)),
  prompt TEXT,
  answer_type TEXT CHECK (answer_type IN (
    'boolean', 'single_choice', 'multi_choice', 'text', 'date', 'identity', 'identity_list'
  )),
  choices TEXT CHECK (json_valid(choices)),
  fills_role TEXT,
  feeds_milestone TEXT,
  help TEXT,
  requires_note INTEGER CHECK (requires_note IS NULL
    OR (requires_note IN (0, 1) AND kind IN ('deliverable', 'action'))),
  PRIMARY KEY (graph_id, key),
  CHECK (kind IN ('deliverable', 'action') OR (estimate IS NULL AND placeholder IS NULL)),
  CHECK (kind = 'deliverable' OR requires_artifact IS NULL),
  CHECK (kind = 'milestone' OR (is_final IS NULL AND auto_reach IS NULL)),
  CHECK (kind = 'group'
    OR (opens_at IS NULL AND closes_at IS NULL AND gates IS NULL AND closes IS NULL)),
  CHECK ((kind = 'decision') = (prompt IS NOT NULL AND answer_type IS NOT NULL)),
  CHECK (kind = 'decision' OR help IS NULL),
  CHECK (choices IS NULL OR answer_type IN ('single_choice', 'multi_choice')),
  CHECK (fills_role IS NULL OR answer_type IN ('identity', 'identity_list')),
  CHECK (feeds_milestone IS NULL OR answer_type = 'date')
) STRICT;

INSERT INTO nodes_new (
  graph_id, key, parent_key, id, kind, title, description, weight, relevant_when, due_by,
  not_before, estimate, placeholder, requires_artifact, is_final, auto_reach, opens_at,
  closes_at, gates, closes, prompt, answer_type, choices, fills_role, feeds_milestone, help,
  requires_note
)
  SELECT graph_id, key, parent_key, id, kind, title, description, weight, relevant_when, due_by,
         not_before, estimate, placeholder, requires_artifact, is_final, auto_reach, opens_at,
         closes_at, gates, closes, prompt,
         CASE answer_type WHEN 'entity' THEN 'identity' WHEN 'entity_list' THEN 'identity_list'
           ELSE answer_type END,
         choices, fills_role, feeds_milestone, help, requires_note
  FROM nodes;

DROP TABLE nodes;
ALTER TABLE nodes_new RENAME TO nodes;
CREATE INDEX nodes_siblings ON nodes (graph_id, parent_key, id);

UPDATE events SET event_type = CASE event_type
  WHEN 'entity_created' THEN 'identity_created'
  WHEN 'entity_edited' THEN 'identity_edited'
  WHEN 'entities_merged' THEN 'identities_merged'
  ELSE event_type END;

DROP TABLE identity_key_map;
