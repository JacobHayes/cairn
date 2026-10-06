//! Secrets Cairn mints (session tokens, agent tokens, authorization codes) and the digests
//! the store keeps in their place (ARCHITECTURE, Store trait: only a token's hash is
//! stored, so a leaked database does not leak a usable token).

use std::fmt::Write as _;

use cairn_schema::Prefixed;
use cairn_store::SecretHash;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::error::AuthError;

/// Random bytes in every secret: 256 bits, past any guessing.
const SECRET_BYTES: usize = 32;
/// Random bytes in a minted user or agent id: 128 bits, so ids never collide.
const ID_BYTES: usize = 16;

/// What a secret is, which its prefix names, so a presented token is routed to the one
/// place that can check it and a session cookie is never accepted as a bearer token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretKind {
    /// A browser session (the session cookie).
    Session,
    /// An agent token (a bearer token, H2).
    Agent,
    /// An authorization code of the built-in OAuth server, single use.
    Code,
    /// A pending browser step: an OAuth consent awaiting its form post.
    Step,
}

impl SecretKind {
    /// The prefix every secret of this kind starts with.
    #[must_use]
    pub fn prefix(self) -> &'static str {
        match self {
            SecretKind::Session => "cairn_session_",
            SecretKind::Agent => "cairn_agent_",
            SecretKind::Code => "cairn_code_",
            SecretKind::Step => "cairn_step_",
        }
    }

    /// Whether `text` is shaped like a secret of this kind.
    #[must_use]
    pub fn matches(self, text: &str) -> bool {
        text.strip_prefix(self.prefix()).is_some_and(|rest| {
            rest.len() == SECRET_BYTES * 2 && rest.bytes().all(|b| b.is_ascii_hexdigit())
        })
    }
}

/// The prefix every secret Cairn mints starts with: a bearer token that has it is one of
/// Cairn's own, never a provider's.
pub const SECRET_PREFIX: &str = "cairn_";

/// A freshly minted secret: shown to its holder once, stored only as [`digest`].
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// A new random secret of `kind`.
    ///
    /// # Errors
    ///
    /// When the operating system's random source fails.
    ///
    /// # Panics
    ///
    /// When the minted text does not have its kind's shape, a bug in this module.
    pub fn mint(kind: SecretKind) -> Result<Self, AuthError> {
        let mut text = kind.prefix().to_owned();
        text.push_str(&random_hex(SECRET_BYTES)?);
        assert!(kind.matches(&text), "a minted secret has its kind's shape");
        Ok(Self(text))
    }

    /// The secret itself, for the one response that hands it over.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// What the store keeps in its place.
    #[must_use]
    pub fn digest(&self) -> SecretHash {
        digest(&self.0)
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Secret(..)")
    }
}

/// The SHA-256 digest of `text`, lower-case hex, as the store keys secrets.
#[must_use]
pub fn digest(text: &str) -> SecretHash {
    match hex(&Sha256::digest(text.as_bytes())).parse::<SecretHash>() {
        Ok(digest) => digest,
        Err(error) => unreachable!("a SHA-256 hex digest parses as a secret hash: {error}"),
    }
}

/// Whether two secrets are equal, compared in constant time over their digests so neither
/// length nor content leaks through timing.
#[must_use]
pub fn secrets_equal(presented: &str, held: &str) -> bool {
    let presented = Sha256::digest(presented.as_bytes());
    let held = Sha256::digest(held.as_bytes());
    presented.ct_eq(&held).into()
}

/// `bytes` random bytes from the operating system, lower-case hex.
///
/// # Errors
///
/// When the operating system's random source fails.
pub fn random_hex(bytes: usize) -> Result<String, AuthError> {
    let mut buffer = vec![0_u8; bytes];
    getrandom::fill(&mut buffer).map_err(|error| AuthError::Random(error.to_string()))?;
    Ok(hex(&buffer))
}

/// A new random id with `K`'s prefix: a user or an agent.
///
/// # Errors
///
/// When the operating system's random source fails.
pub fn mint_id<K: Prefixed>() -> Result<K, AuthError> {
    let text = format!("{}{}", K::PREFIX, random_hex(ID_BYTES)?);
    match text.parse() {
        Ok(id) => Ok(id),
        Err(error) => unreachable!("a minted id parses: {error}"),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minted_secret_has_its_kind_and_is_stored_as_its_digest() {
        for kind in [
            SecretKind::Session,
            SecretKind::Agent,
            SecretKind::Code,
            SecretKind::Step,
        ] {
            let secret = Secret::mint(kind).unwrap();
            assert!(kind.matches(secret.expose()));
            assert!(secret.expose().starts_with(SECRET_PREFIX));
            assert_ne!(secret.digest().as_str(), secret.expose());
            assert_eq!(secret.digest(), digest(secret.expose()));
            assert_ne!(Secret::mint(kind).unwrap(), secret);
        }
        let session = Secret::mint(SecretKind::Session).unwrap();
        assert!(!SecretKind::Agent.matches(session.expose()));
        assert!(!format!("{session:?}").contains(session.expose()));
    }

    #[test]
    fn secrets_compare_equal_only_when_identical() {
        assert!(secrets_equal("open sesame", "open sesame"));
        for other in ["open sesame ", "open", "", "OPEN SESAME"] {
            assert!(!secrets_equal(other, "open sesame"), "{other:?}");
        }
    }
}
