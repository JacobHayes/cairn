-- The store's tables (ARCHITECTURE, Storage > Schema outline; PRD A14, Non-functional:
-- Structured storage): relational rows, no graph as a blob. Every table is STRICT; CHECK
-- constraints hold enums, flags, and non-negative numbers; foreign keys are deferred to the
-- commit, since a change set may write a child record before its parent. Structured field
-- values the schema crate validates before they are written (conditions, date rules,
-- choices, answers, a bypass, a proposal's mutations and review items, an event's subject
-- and delta) are JSON columns. Timestamps are nanoseconds since the Unix epoch; calendar
-- dates are ISO text.
--
-- Uniqueness a patch may pass through on its way to a valid graph (sibling ids, emails) is
-- checked at the end of the commit rather than by a unique index: SQLite has no deferred
-- unique constraint (DECISIONS.md).

-- Domains and their revisions (H5). The first write of every commit is its domain's row.

CREATE TABLE deployment (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  revision INTEGER NOT NULL CHECK (revision >= 0)
) STRICT;

INSERT INTO deployment (id, revision) VALUES (1, 0);

CREATE TABLE routes (
  id TEXT PRIMARY KEY,
  revision INTEGER NOT NULL CHECK (revision >= 1),
  name TEXT NOT NULL CHECK (length(name) > 0),
  description TEXT,
  retired INTEGER NOT NULL CHECK (retired IN (0, 1))
) STRICT;

CREATE TABLE route_drafts (
  route_id TEXT PRIMARY KEY REFERENCES routes (id) DEFERRABLE INITIALLY DEFERRED,
  extends INTEGER CHECK (extends >= 1)
) STRICT;

CREATE TABLE route_versions (
  route_id TEXT NOT NULL REFERENCES routes (id) DEFERRABLE INITIALLY DEFERRED,
  version_number INTEGER NOT NULL CHECK (version_number >= 1),
  published_at INTEGER NOT NULL,
  PRIMARY KEY (route_id, version_number)
) STRICT;

CREATE TABLE journeys (
  id TEXT PRIMARY KEY,
  revision INTEGER NOT NULL CHECK (revision >= 1),
  name TEXT NOT NULL CHECK (length(name) > 0),
  description TEXT,
  status TEXT NOT NULL CHECK (status IN ('active', 'completed', 'archived')),
  lineage_route TEXT,
  lineage_version INTEGER CHECK (lineage_version >= 1),
  created_at INTEGER NOT NULL,
  created_on TEXT NOT NULL,
  CHECK ((lineage_route IS NULL) = (lineage_version IS NULL))
) STRICT;

-- A19: a hard-deleted journey's id, never reused.
CREATE TABLE deleted_journeys (
  id TEXT PRIMARY KEY,
  deleted_at INTEGER NOT NULL
) STRICT;

-- Graphs: a journey's, a route's draft, and each published version. Content and state rows
-- belong to one.

CREATE TABLE graphs (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('journey', 'route_draft', 'route_version')),
  journey_id TEXT UNIQUE REFERENCES journeys (id) DEFERRABLE INITIALLY DEFERRED,
  route_id TEXT REFERENCES routes (id) DEFERRABLE INITIALLY DEFERRED,
  version_number INTEGER CHECK (version_number >= 1),
  default_owner TEXT,
  CHECK ((kind = 'journey') = (journey_id IS NOT NULL)),
  CHECK ((kind = 'journey') = (route_id IS NULL)),
  CHECK ((kind = 'route_version') = (version_number IS NOT NULL))
) STRICT;

-- A1a: one row per node, a column per field, each kind-restricted field only on the kinds
-- that carry it.
CREATE TABLE nodes (
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
    'boolean', 'single_choice', 'multi_choice', 'text', 'date', 'entity', 'entity_list'
  )),
  choices TEXT CHECK (json_valid(choices)),
  fills_role TEXT,
  feeds_milestone TEXT,
  help TEXT,
  PRIMARY KEY (graph_id, key),
  CHECK (kind IN ('deliverable', 'action') OR (estimate IS NULL AND placeholder IS NULL)),
  CHECK (kind = 'deliverable' OR requires_artifact IS NULL),
  CHECK (kind = 'milestone' OR (is_final IS NULL AND auto_reach IS NULL)),
  CHECK (kind = 'group'
    OR (opens_at IS NULL AND closes_at IS NULL AND gates IS NULL AND closes IS NULL)),
  CHECK ((kind = 'decision') = (prompt IS NOT NULL AND answer_type IS NOT NULL)),
  CHECK (kind = 'decision' OR help IS NULL),
  CHECK (choices IS NULL OR answer_type IN ('single_choice', 'multi_choice')),
  CHECK (fills_role IS NULL OR answer_type IN ('entity', 'entity_list')),
  CHECK (feeds_milestone IS NULL OR answer_type = 'date')
) STRICT;

CREATE INDEX nodes_siblings ON nodes (graph_id, parent_key, id);

-- A3: explicit edges, `node` requires `requires`.
CREATE TABLE edges (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  node TEXT NOT NULL,
  requires TEXT NOT NULL,
  PRIMARY KEY (graph_id, node, requires)
) STRICT;

CREATE TABLE roles (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  key TEXT NOT NULL,
  id TEXT NOT NULL,
  title TEXT,
  multi INTEGER NOT NULL CHECK (multi IN (0, 1)),
  PRIMARY KEY (graph_id, key)
) STRICT;

CREATE TABLE participation_kinds (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  key TEXT NOT NULL,
  id TEXT NOT NULL,
  title TEXT,
  multi INTEGER NOT NULL CHECK (multi IN (0, 1)),
  PRIMARY KEY (graph_id, key)
) STRICT;

-- E2: a node's participation of one kind: a role, or explicit entities (none is explicit
-- "nobody").
CREATE TABLE participations (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  node TEXT NOT NULL,
  kind TEXT NOT NULL,
  role TEXT,
  PRIMARY KEY (graph_id, node, kind)
) STRICT;

CREATE TABLE participation_entities (
  graph_id TEXT NOT NULL,
  node TEXT NOT NULL,
  kind TEXT NOT NULL,
  entity TEXT NOT NULL,
  PRIMARY KEY (graph_id, node, kind, entity),
  FOREIGN KEY (graph_id, node, kind) REFERENCES participations (graph_id, node, kind)
    DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE INDEX participation_entities_entity ON participation_entities (entity);

-- A10: route-authored guidance on a node, in the order written.
CREATE TABLE resources (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  node TEXT NOT NULL,
  key TEXT NOT NULL,
  position INTEGER NOT NULL CHECK (position >= 0),
  title TEXT,
  type TEXT NOT NULL
    CHECK (type IN ('tip', 'template', 'example', 'reference', 'message_draft')),
  body TEXT NOT NULL,
  PRIMARY KEY (graph_id, node, key)
) STRICT;

-- Every key a graph retired, never reused.
CREATE TABLE retired_keys (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  key TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('node', 'role', 'kind')),
  PRIMARY KEY (graph_id, key)
) STRICT;

-- Journey state.

CREATE TABLE node_states (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  node TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN (
    'todo', 'active', 'done', 'skipped', 'open', 'decided', 'pending', 'reached', 'derived'
  )),
  provenance TEXT NOT NULL CHECK (provenance IN ('from_route', 'local', 'orphaned')),
  atomic INTEGER NOT NULL CHECK (atomic IN (0, 1)),
  started_on TEXT,
  finished_on TEXT,
  skip_reason TEXT,
  PRIMARY KEY (graph_id, node)
) STRICT;

-- B4: a journey's local edit to a route-copied node: a field, an edge to a requirement, a
-- participation kind, a resource, or its shape (B7: kind or answer type, with an empty target).
CREATE TABLE local_edits (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  node TEXT NOT NULL,
  aspect TEXT NOT NULL
    CHECK (aspect IN ('field', 'requires', 'participation', 'resource', 'shape')),
  target TEXT NOT NULL CHECK ((aspect = 'shape') = (target = '')),
  PRIMARY KEY (graph_id, node, aspect, target)
) STRICT;

CREATE TABLE answers (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  decision TEXT NOT NULL,
  answer_type TEXT NOT NULL CHECK (answer_type IN (
    'boolean', 'single_choice', 'multi_choice', 'text', 'date', 'entity', 'entity_list'
  )),
  value TEXT NOT NULL CHECK (json_valid(value)),
  PRIMARY KEY (graph_id, decision)
) STRICT;

-- The entities an entity or entity-list answer names, for the journeys referencing an
-- entity (E6).
CREATE TABLE answer_entities (
  graph_id TEXT NOT NULL,
  decision TEXT NOT NULL,
  entity TEXT NOT NULL,
  PRIMARY KEY (graph_id, decision, entity),
  FOREIGN KEY (graph_id, decision) REFERENCES answers (graph_id, decision)
    DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE INDEX answer_entities_entity ON answer_entities (entity);

-- E3: a role filled directly (none is explicit "nobody").
CREATE TABLE role_fills (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  role TEXT NOT NULL,
  PRIMARY KEY (graph_id, role)
) STRICT;

CREATE TABLE role_fill_entities (
  graph_id TEXT NOT NULL,
  role TEXT NOT NULL,
  entity TEXT NOT NULL,
  PRIMARY KEY (graph_id, role, entity),
  FOREIGN KEY (graph_id, role) REFERENCES role_fills (graph_id, role)
    DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE INDEX role_fill_entities_entity ON role_fill_entities (entity);

CREATE TABLE pins (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  node TEXT NOT NULL,
  date TEXT NOT NULL,
  PRIMARY KEY (graph_id, node)
) STRICT;

-- B6: a snooze until a date or until a node completes.
CREATE TABLE snoozes (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  node TEXT NOT NULL,
  until_date TEXT,
  until_node TEXT,
  PRIMARY KEY (graph_id, node),
  CHECK ((until_date IS NULL) <> (until_node IS NULL))
) STRICT;

-- Overrides: force include, keep (D1a), and a guard bypass with its guards, reason, and
-- the failures present (D4).
CREATE TABLE overrides (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  node TEXT NOT NULL,
  force_include TEXT,
  keep TEXT,
  bypass TEXT CHECK (json_valid(bypass)),
  PRIMARY KEY (graph_id, node)
) STRICT;

-- B4: route-copied nodes the journey removed.
CREATE TABLE tombstones (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  node TEXT NOT NULL,
  PRIMARY KEY (graph_id, node)
) STRICT;

-- G1: notes and links, on a node or on the journey itself.
CREATE TABLE annotations (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  key TEXT NOT NULL,
  node TEXT,
  title TEXT,
  type TEXT NOT NULL CHECK (type IN ('note', 'artifact', 'reference', 'conversation')),
  content TEXT NOT NULL,
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  edited_at INTEGER,
  PRIMARY KEY (graph_id, key)
) STRICT;

-- The deployment's entities (E6, H3).

CREATE TABLE entities (
  key TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK (length(name) > 0)
) STRICT;

CREATE TABLE entity_emails (
  entity TEXT NOT NULL REFERENCES entities (key) DEFERRABLE INITIALLY DEFERRED,
  email TEXT NOT NULL,
  PRIMARY KEY (entity, email)
) STRICT;

CREATE INDEX entity_emails_email ON entity_emails (email);

CREATE TABLE entity_aliases (
  alias TEXT PRIMARY KEY,
  entity TEXT NOT NULL REFERENCES entities (key) DEFERRABLE INITIALLY DEFERRED
) STRICT;

-- I6: proposals, with their own editing revision (H5).
CREATE TABLE proposals (
  id TEXT PRIMARY KEY,
  destination_kind TEXT NOT NULL CHECK (destination_kind IN ('journey', 'route', 'deployment')),
  destination_id TEXT,
  revision INTEGER NOT NULL CHECK (revision >= 0),
  status TEXT NOT NULL CHECK (status IN ('open', 'applied', 'discarded')),
  title TEXT NOT NULL,
  description TEXT,
  destination_revision INTEGER NOT NULL CHECK (destination_revision >= 0),
  mutations TEXT NOT NULL CHECK (json_valid(mutations)),
  items TEXT NOT NULL CHECK (json_valid(items)),
  proposing_agent TEXT,
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  CHECK ((destination_kind = 'deployment') = (destination_id IS NULL))
) STRICT;

-- H5: what a resubmitted patch id is answered from, and what each commit advanced.
CREATE TABLE patch_receipts (
  patch_id TEXT PRIMARY KEY,
  domain_kind TEXT NOT NULL CHECK (domain_kind IN ('journey', 'route', 'deployment')),
  domain_id TEXT,
  proposal_id TEXT,
  content_hash TEXT NOT NULL CHECK (length(content_hash) = 64),
  revision INTEGER NOT NULL CHECK (revision >= 1),
  deployment_revision INTEGER CHECK (deployment_revision >= 1),
  CHECK ((domain_kind = 'deployment') = (domain_id IS NULL))
) STRICT;

-- J1: the append-only log, one row per mutation; `seq` orders history.
CREATE TABLE events (
  seq INTEGER PRIMARY KEY,
  patch_id TEXT NOT NULL,
  ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
  log_kind TEXT NOT NULL CHECK (log_kind IN ('journey', 'route', 'deployment')),
  log_id TEXT,
  event_type TEXT NOT NULL,
  actor_user TEXT NOT NULL,
  agent TEXT,
  confirming_user TEXT,
  subject TEXT NOT NULL CHECK (json_valid(subject)),
  at INTEGER NOT NULL,
  note TEXT,
  delta TEXT NOT NULL CHECK (json_valid(delta)),
  UNIQUE (patch_id, ordinal),
  CHECK ((log_kind = 'deployment') = (log_id IS NULL))
) STRICT;

CREATE INDEX events_log ON events (log_kind, log_id, seq);
CREATE INDEX events_actor ON events (actor_user, seq);
CREATE INDEX events_at ON events (at);

-- J5: the nodes each event is about or wrote on, for the history of one node.
CREATE TABLE event_nodes (
  seq INTEGER NOT NULL REFERENCES events (seq) DEFERRABLE INITIALLY DEFERRED,
  node TEXT NOT NULL,
  PRIMARY KEY (seq, node)
) STRICT;

CREATE INDEX event_nodes_node ON event_nodes (node, seq);
