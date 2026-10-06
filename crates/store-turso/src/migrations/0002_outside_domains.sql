-- Records outside every domain, with no events (ARCHITECTURE, Store trait; Schema
-- outline): auth state, which the auth crate logs itself, and assistant conversations.
-- Secrets are held only as SHA-256 digests (lower-case hex), so a leaked database does not
-- leak a usable session, token, or code. Users are never deleted, so the references to
-- them need no deferral.

CREATE TABLE users (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK (length(name) > 0),
  created_at INTEGER NOT NULL
) STRICT;

CREATE TABLE user_identities (
  provider TEXT NOT NULL,
  subject TEXT NOT NULL,
  user_id TEXT NOT NULL REFERENCES users (id),
  linked_at INTEGER NOT NULL,
  PRIMARY KEY (provider, subject)
) STRICT;

CREATE INDEX user_identities_user ON user_identities (user_id);

-- H3: the emails a provider marked verified.
CREATE TABLE identity_emails (
  provider TEXT NOT NULL,
  subject TEXT NOT NULL,
  email TEXT NOT NULL,
  PRIMARY KEY (provider, subject, email),
  FOREIGN KEY (provider, subject) REFERENCES user_identities (provider, subject)
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
  event TEXT NOT NULL CHECK (event IN (
    'user_created', 'identity_linked', 'identity_unlinked', 'token_minted', 'token_revoked'
  )),
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
