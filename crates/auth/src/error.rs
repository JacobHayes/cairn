//! The auth crate's errors: refusals of a request, failures of an auth operation, and
//! configurations a provider will not start with.

use std::fmt;

use cairn_schema::UserId;
use cairn_store::StoreError;

/// Why a request's credentials were refused. A refusal decides the request: no other
/// provider is asked to accept it instead (3.2, Security).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// A presented credential is not valid: unknown, expired, revoked, or wrong.
    Credential,
    /// A credential accepted only from the local machine came from elsewhere.
    Peer,
    /// The store or an identity provider failed, so the credential could not be checked.
    Unavailable(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Credential => formatter.write_str("the presented credential is not valid"),
            Refusal::Peer => formatter.write_str("this credential is accepted only locally"),
            Refusal::Unavailable(reason) => write!(formatter, "authentication failed: {reason}"),
        }
    }
}

impl std::error::Error for Refusal {}

/// A failed auth operation: a sign-in, a link, a token mint or revoke.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthError {
    /// The store failed.
    Store(StoreError),
    /// The operating system's random source failed.
    Random(String),
    /// An identity provider failed or answered with something invalid.
    Upstream(String),
    /// The identity already belongs to another user; merging users is `Later` (PRD).
    IdentityHeldByAnotherUser {
        /// The user holding it.
        holder: UserId,
    },
    /// An agent tried what only a user may: mint a token or authorize a client (H2).
    AgentNotAllowed,
    /// The token named is not one of the user's.
    NoSuchToken,
}

impl fmt::Display for AuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::Store(error) => write!(formatter, "{error}"),
            AuthError::Random(reason) => write!(formatter, "random source failed: {reason}"),
            AuthError::Upstream(reason) => write!(formatter, "identity provider: {reason}"),
            AuthError::IdentityHeldByAnotherUser { holder } => {
                write!(formatter, "the identity already belongs to user {holder}")
            }
            AuthError::AgentNotAllowed => formatter.write_str("an agent cannot do this"),
            AuthError::NoSuchToken => formatter.write_str("no such agent token"),
        }
    }
}

impl std::error::Error for AuthError {}

impl From<StoreError> for AuthError {
    fn from(error: StoreError) -> Self {
        AuthError::Store(error)
    }
}

impl From<AuthError> for Refusal {
    fn from(error: AuthError) -> Self {
        Refusal::Unavailable(error.to_string())
    }
}

/// A provider configuration that is refused at start-up, naming the provider and the rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigError {
    /// The provider.
    pub provider: String,
    /// The rule it breaks.
    pub reason: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "auth provider {}: {}",
            self.provider, self.reason
        )
    }
}

impl std::error::Error for ConfigError {}
