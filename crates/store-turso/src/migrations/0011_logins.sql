-- A user's sign-ins are logins, so "identity" names only the people and teams journeys refer
-- to (migration 0011 goes on to rename those). The login tables are rebuilt under their new
-- names; the log's CHECK lists the renamed event, and `identity_unlinked`, which nothing ever
-- emitted, is gone (a row holding it fails the copy, loudly, rather than being rewritten).

CREATE TABLE user_logins (
  provider TEXT NOT NULL,
  subject TEXT NOT NULL,
  user_id TEXT NOT NULL REFERENCES users (id),
  linked_at INTEGER NOT NULL,
  PRIMARY KEY (provider, subject)
) STRICT;

INSERT INTO user_logins (provider, subject, user_id, linked_at)
  SELECT provider, subject, user_id, linked_at FROM user_identities;

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

INSERT INTO login_emails (provider, subject, email)
  SELECT provider, subject, email FROM identity_emails;

DROP TABLE identity_emails;
DROP TABLE user_identities;

CREATE TABLE auth_log_new (
  seq INTEGER PRIMARY KEY,
  at INTEGER NOT NULL,
  event TEXT NOT NULL CHECK (event IN (
    'user_created', 'login_linked', 'token_minted', 'token_revoked'
  )),
  user_id TEXT NOT NULL,
  subject TEXT
) STRICT;

INSERT INTO auth_log_new (seq, at, event, user_id, subject)
  SELECT seq, at, CASE event WHEN 'identity_linked' THEN 'login_linked' ELSE event END,
         user_id, subject
  FROM auth_log;

DROP TABLE auth_log;
ALTER TABLE auth_log_new RENAME TO auth_log;
CREATE INDEX auth_log_user ON auth_log (user_id, seq);
