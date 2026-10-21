-- The store's tables (ARCHITECTURE, Storage > Schema outline; PRD A14, Non-functional:
-- Structured storage): relational rows, no graph as a blob. Every table is STRICT; CHECK
-- constraints hold enums, flags, and non-negative numbers; foreign keys are deferred to the
-- commit, since a change set may write a child record before its parent. Structured field
-- values the schema crate validates before they are written (conditions, date rules,
-- choices, answers, a bypass, a proposal's mutations and review items, an event's subject
-- and delta, an insertion's maps and members) are JSON columns. Timestamps are nanoseconds
-- since the Unix epoch; calendar dates are ISO text.
--
-- Uniqueness a patch may pass through on its way to a valid graph (sibling ids, emails) is
-- checked at the end of the commit rather than by a unique index: SQLite has no deferred
-- unique constraint
-- (decisions/2026-10-06-uniqueness-a-patch-may-pass-through-is-checked-at-the-end.md).
--
-- This is the schema migrations 1 to 13 of the Turso store left, as one file: a database
-- converted from a release on that store is at migration 13 or later already, and a new
-- one starts there (decisions/2026-10-21-the-store-is-sqlite-with-one-writer.md).

-- Domains and their revisions (H5). The first write of every commit is its domain's row.

CREATE TABLE deployment (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  revision INTEGER NOT NULL CHECK (revision >= 0)
) STRICT;

INSERT INTO deployment (id, revision) VALUES (1, 0);

-- A21: a route is a process or a segment.
CREATE TABLE routes (
  id TEXT PRIMARY KEY,
  revision INTEGER NOT NULL CHECK (revision >= 1),
  name TEXT NOT NULL CHECK (length(name) > 0),
  description TEXT,
  retired INTEGER NOT NULL CHECK (retired IN (0, 1)),
  kind TEXT NOT NULL DEFAULT 'process' CHECK (kind IN ('process', 'segment'))
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
  answer_type TEXT CHECK (answer_type IN ('boolean', 'single_choice', 'multi_choice', 'text',
    'date', 'identity', 'identity_list')),
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

-- E2: a node's participation of one kind: a role, or explicit identities (none is explicit
-- "nobody").
CREATE TABLE participations (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  node TEXT NOT NULL,
  kind TEXT NOT NULL,
  role TEXT,
  PRIMARY KEY (graph_id, node, kind)
) STRICT;

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

-- B13: one placement of a segment version in a graph. The maps and the members are JSON the
-- schema validates before it writes, as condition trees are.
CREATE TABLE insertions (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  key TEXT NOT NULL,
  segment TEXT NOT NULL,
  version INTEGER NOT NULL CHECK (version >= 1),
  parent_key TEXT,
  roles TEXT NOT NULL CHECK (json_valid(roles)),
  kinds TEXT NOT NULL CHECK (json_valid(kinds)),
  nodes TEXT NOT NULL CHECK (json_valid(nodes)),
  PRIMARY KEY (graph_id, key)
) STRICT;

CREATE INDEX insertions_by_segment ON insertions (segment, version);

-- Journey state.

CREATE TABLE node_states (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  node TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('todo', 'active', 'done', 'skipped', 'open', 'decided',
    'pending', 'reached', 'derived')),
  provenance TEXT NOT NULL
    CHECK (provenance IN ('from_route', 'from_segment', 'local', 'orphaned')),
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

-- B2: an answer, with the rationale it was given with (markdown, null when it gave none).
CREATE TABLE answers (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  decision TEXT NOT NULL,
  answer_type TEXT NOT NULL CHECK (answer_type IN ('boolean', 'single_choice', 'multi_choice',
    'text', 'date', 'identity', 'identity_list')),
  value TEXT NOT NULL CHECK (json_valid(value)),
  rationale TEXT CHECK (rationale IS NULL OR trim(rationale) <> ''),
  PRIMARY KEY (graph_id, decision)
) STRICT;

-- E3: a role filled directly (none is explicit "nobody").
CREATE TABLE role_fills (
  graph_id TEXT NOT NULL REFERENCES graphs (id) DEFERRABLE INITIALLY DEFERRED,
  role TEXT NOT NULL,
  PRIMARY KEY (graph_id, role)
) STRICT;

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

-- The references journeys and routes hold, by identity (E6): what the graphs referring to an
-- identity are found by.

CREATE TABLE answer_identities (
  graph_id TEXT NOT NULL,
  decision TEXT NOT NULL,
  identity TEXT NOT NULL,
  PRIMARY KEY (graph_id, decision, identity),
  FOREIGN KEY (graph_id, decision) REFERENCES answers (graph_id, decision)
    DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE INDEX answer_identities_identity ON answer_identities (identity);

CREATE TABLE role_fill_identities (
  graph_id TEXT NOT NULL,
  role TEXT NOT NULL,
  identity TEXT NOT NULL,
  PRIMARY KEY (graph_id, role, identity),
  FOREIGN KEY (graph_id, role) REFERENCES role_fills (graph_id, role)
    DEFERRABLE INITIALLY DEFERRED
) STRICT;

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

CREATE INDEX participation_identities_identity ON participation_identities (identity);

-- The identities a node's condition compares an identity or identity-list decision with, so
-- the graphs referring to an identity are found exactly instead of by searching condition
-- text. Maintained by every commit that writes nodes.
CREATE TABLE condition_identities (
  graph_id TEXT NOT NULL,
  node TEXT NOT NULL,
  identity TEXT NOT NULL,
  PRIMARY KEY (graph_id, node, identity)
) STRICT;

CREATE INDEX condition_identities_identity ON condition_identities (identity);

-- The deployment's identities (E6, H3): the people and teams journeys refer to, under opaque
-- keys. An identity may have no name, only emails.

CREATE TABLE identities (
  key TEXT PRIMARY KEY,
  name TEXT CHECK (name IS NULL OR length(name) > 0)
) STRICT;

CREATE TABLE identity_emails (
  identity TEXT NOT NULL REFERENCES identities (key) DEFERRABLE INITIALLY DEFERRED,
  email TEXT NOT NULL,
  PRIMARY KEY (identity, email)
) STRICT;

CREATE INDEX identity_emails_email ON identity_emails (email);

CREATE TABLE identity_aliases (
  alias TEXT PRIMARY KEY,
  identity TEXT NOT NULL REFERENCES identities (key) DEFERRABLE INITIALLY DEFERRED
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

-- C14, C16: the open proposals of a destination, and of a creator, newest first. The first
-- also serves the delete of every proposal of a journey, whatever its status (A19).
CREATE INDEX proposals_by_destination
  ON proposals (destination_kind, destination_id, status, created_at, id);
CREATE INDEX proposals_by_creator ON proposals (created_by, status, created_at, id);

-- H5: what a resubmitted patch id is answered from, and what each commit advanced: the
-- revisions it left the proposals it wrote at, and what it wrote in the deployment, so a
-- stale patch is told what intervened even after a hard delete has removed the events (A19).
CREATE TABLE patch_receipts (
  patch_id TEXT PRIMARY KEY,
  domain_kind TEXT NOT NULL CHECK (domain_kind IN ('journey', 'route', 'deployment')),
  domain_id TEXT,
  proposal_id TEXT,
  content_hash TEXT NOT NULL CHECK (length(content_hash) = 64),
  revision INTEGER NOT NULL CHECK (revision >= 1),
  deployment_revision INTEGER CHECK (deployment_revision >= 1),
  proposals TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(proposals)),
  deployment_touched TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(deployment_touched)),
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

-- Records outside every domain, with no events (ARCHITECTURE, Store trait; Schema outline):
-- auth state, which the auth crate logs itself, and assistant conversations. Secrets are
-- held only as SHA-256 digests (lower-case hex), so a leaked database does not leak a usable
-- session, token, or code. Users are never deleted, so the references to them need no
-- deferral.

CREATE TABLE users (
  id TEXT PRIMARY KEY,
  created_at INTEGER NOT NULL
) STRICT;

-- A user's sign-ins, each with the name its provider gave (H3).
CREATE TABLE user_logins (
  provider TEXT NOT NULL,
  subject TEXT NOT NULL,
  user_id TEXT NOT NULL REFERENCES users (id),
  linked_at INTEGER NOT NULL,
  name TEXT CHECK (name IS NULL OR length(name) > 0),
  PRIMARY KEY (provider, subject)
) STRICT;

CREATE INDEX user_logins_user ON user_logins (user_id);

-- H3: the emails a provider marked verified.
CREATE TABLE login_emails (
  provider TEXT NOT NULL,
  subject TEXT NOT NULL,
  email TEXT NOT NULL,
  PRIMARY KEY (provider, subject, email),
  FOREIGN KEY (provider, subject) REFERENCES user_logins (provider, subject)
    DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE TABLE sessions (
  token_hash TEXT PRIMARY KEY CHECK (length(token_hash) = 64),
  user_id TEXT NOT NULL REFERENCES users (id),
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL
) STRICT;

CREATE TABLE oauth_transient (
  key_hash TEXT PRIMARY KEY CHECK (length(key_hash) = 64),
  kind TEXT NOT NULL CHECK (kind IN ('authorization_code', 'pkce_verifier', 'login_state')),
  payload TEXT NOT NULL,
  expires_at INTEGER NOT NULL
) STRICT;

CREATE TABLE agent_tokens (
  agent TEXT PRIMARY KEY,
  token_hash TEXT NOT NULL UNIQUE CHECK (length(token_hash) = 64),
  name TEXT NOT NULL,
  user_id TEXT NOT NULL REFERENCES users (id),
  created_at INTEGER NOT NULL,
  revoked_at INTEGER
) STRICT;

CREATE INDEX agent_tokens_user ON agent_tokens (user_id);

CREATE TABLE auth_log (
  seq INTEGER PRIMARY KEY,
  at INTEGER NOT NULL,
  event TEXT NOT NULL
    CHECK (event IN ('user_created', 'login_linked', 'token_minted', 'token_revoked')),
  user_id TEXT NOT NULL,
  subject TEXT
) STRICT;

CREATE INDEX auth_log_user ON auth_log (user_id, seq);

CREATE TABLE conversations (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  title TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
) STRICT;

CREATE INDEX conversations_user ON conversations (user_id, id);

CREATE TABLE conversation_messages (
  conversation TEXT NOT NULL REFERENCES conversations (id) DEFERRABLE INITIALLY DEFERRED,
  position INTEGER NOT NULL CHECK (position >= 0),
  author TEXT NOT NULL CHECK (author IN ('user', 'assistant', 'tool')),
  at INTEGER NOT NULL,
  content TEXT NOT NULL,
  PRIMARY KEY (conversation, position)
) STRICT;
