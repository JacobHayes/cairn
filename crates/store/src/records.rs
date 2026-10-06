//! Records outside every domain, with no events (ARCHITECTURE, Store trait; Schema
//! outline): auth state (users, their identities, login sessions, transient OAuth state,
//! agent tokens, and the auth log) and assistant conversations. Plain records the auth and
//! assistant crates build their own types over (3.2, 4.4). Secrets are never stored: a
//! session or agent token, or an OAuth code, is held only as its SHA-256 digest, so a
//! leaked database does not leak a usable secret.

use std::fmt;
use std::str::FromStr;

use cairn_schema::{AgentId, ConversationId, Email, Markdown, Slug, Timestamp, Title, UserId};
use serde::{Deserialize, Serialize};

use crate::query::PageSize;

/// A secret's SHA-256 digest, lower-case hex.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SecretHash(String);

impl SecretHash {
    /// The hex digest.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for SecretHash {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let hex = text.len() == 64
            && text
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
        if hex {
            Ok(Self(text.to_owned()))
        } else {
            Err(format!("{text:?} is not a lower-case hex SHA-256 digest"))
        }
    }
}

impl fmt::Display for SecretHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// An authenticated account (H1; PRD glossary, User).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserRecord {
    /// The user.
    pub id: UserId,
    /// The name shown for the user.
    pub name: Title,
    /// When the user first signed in.
    pub created_at: Timestamp,
}

/// One of a user's identities, keyed by provider and subject (ARCHITECTURE, Auth).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityRecord {
    /// The provider, by its configured name.
    pub provider: Slug,
    /// The provider's subject for the account.
    pub subject: Title,
    /// The user it signs in as.
    pub user: UserId,
    /// The emails the provider marked verified at the last sign-in (H3).
    pub verified_emails: std::collections::BTreeSet<Email>,
    /// When it was linked.
    pub linked_at: Timestamp,
}

/// A login session (H1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionRecord {
    /// The session token's digest.
    pub token_hash: SecretHash,
    /// The user signed in.
    pub user: UserId,
    /// When it started.
    pub created_at: Timestamp,
    /// When it lapses.
    pub expires_at: Timestamp,
}

/// What a piece of transient OAuth state is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuthStateKind {
    /// An authorization code awaiting exchange.
    AuthorizationCode,
    /// A PKCE verifier awaiting its callback.
    PkceVerifier,
    /// A login's state parameter awaiting its callback.
    LoginState,
}

/// Transient OAuth state: single use, and gone once it expires.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OAuthStateRecord {
    /// The code's or state's digest, which the callback presents.
    pub key_hash: SecretHash,
    /// What it is.
    pub kind: OAuthStateKind,
    /// What the auth crate needs back, as it wrote it.
    pub payload: Markdown,
    /// When it lapses.
    pub expires_at: Timestamp,
}

/// A long-lived agent token (ARCHITECTURE, Auth: shown once when minted, only its digest
/// stored).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentTokenRecord {
    /// The agent the token acts as, named on every change made through it (H2).
    pub agent: AgentId,
    /// The token's digest.
    pub token_hash: SecretHash,
    /// What the user called it.
    pub name: Title,
    /// The user it acts for.
    pub user: UserId,
    /// When it was minted.
    pub created_at: Timestamp,
    /// When it was revoked, if it was.
    pub revoked_at: Option<Timestamp>,
}

/// What an auth log entry records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthEvent {
    /// A user was created on first sign-in.
    UserCreated,
    /// An identity was linked to a user.
    IdentityLinked,
    /// An identity was unlinked.
    IdentityUnlinked,
    /// An agent token was minted.
    TokenMinted,
    /// An agent token was revoked.
    TokenRevoked,
}

/// One auth operation, logged (ARCHITECTURE, Auth: auth operations are logged, not
/// patches).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthLogEntry {
    /// When.
    pub at: Timestamp,
    /// What.
    pub event: AuthEvent,
    /// Whose account.
    pub user: UserId,
    /// What it was about: an identity's provider and subject, or a token's agent.
    pub subject: Option<Title>,
}

/// An auth log entry and its position in the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoggedAuthEntry {
    /// The position: increasing in append order.
    pub seq: u64,
    /// The entry.
    pub entry: AuthLogEntry,
}

/// Auth log filters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuthLogQuery {
    /// Entries about this user.
    pub user: Option<UserId>,
    /// The page starts after this position.
    pub after: Option<u64>,
    /// The page's size.
    pub size: PageSize,
}

/// Who wrote a conversation message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageAuthor {
    /// The user.
    User,
    /// The assistant.
    Assistant,
    /// A tool the assistant called.
    Tool,
}

/// One message of a conversation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationMessage {
    /// Who wrote it.
    pub author: MessageAuthor,
    /// When.
    pub at: Timestamp,
    /// What it says.
    pub content: Markdown,
}

/// An assistant conversation (I5), outside every domain, stored whole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationRecord {
    /// The conversation.
    pub id: ConversationId,
    /// The user holding it.
    pub user: UserId,
    /// A title, once there is one.
    pub title: Option<Title>,
    /// When it started.
    pub created_at: Timestamp,
    /// When it last changed.
    pub updated_at: Timestamp,
    /// The messages, in order.
    pub messages: Vec<ConversationMessage>,
}

/// A conversation in a user's list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationSummary {
    /// The conversation.
    pub id: ConversationId,
    /// Its title.
    pub title: Option<Title>,
    /// When it last changed.
    pub updated_at: Timestamp,
}

impl ConversationRecord {
    /// The summary a list shows.
    #[must_use]
    pub fn summary(&self) -> ConversationSummary {
        ConversationSummary {
            id: self.id.clone(),
            title: self.title.clone(),
            updated_at: self.updated_at,
        }
    }
}
