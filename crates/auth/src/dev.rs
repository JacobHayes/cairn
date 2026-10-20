//! The dev provider (H1: "a simple mode, static token or named dev user"). Local only: it
//! will not start on a listener other machines can reach unless its configuration says so
//! (the off-loopback override, a security choice about this provider, ARCHITECTURE Auth),
//! and without that override it refuses any request that is not from this machine.

use std::collections::BTreeSet;

use async_trait::async_trait;
use cairn_schema::{Email, Login, Slug, Title};

use crate::error::{ConfigError, Refusal};
use crate::provider::{AuthProvider, Listener, Presented, Verdict};
use crate::secret::{SECRET_PREFIX, secrets_equal};

/// The dev provider's configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DevConfig {
    /// The provider's name.
    pub name: Slug,
    /// The dev user's name: the login's subject and name.
    pub user: Title,
    /// Emails the operator vouches for, listed as verified (H3).
    pub verified_emails: BTreeSet<Email>,
    /// With a token, a request signs in by presenting it as a bearer token; without one,
    /// every request is the dev user.
    pub token: Option<String>,
    /// H3: link a new login to the user holding one of its verified emails.
    pub auto_link: bool,
    /// The override: serve requests from other machines too.
    pub allow_off_loopback: bool,
}

/// The dev provider.
#[derive(Debug)]
pub struct DevProvider {
    config: DevConfig,
    login: Login,
}

impl DevProvider {
    /// The dev provider for a server on `listener`.
    ///
    /// # Errors
    ///
    /// The listener is reachable from other machines and the override is off, or the token
    /// is empty or shaped like one of Cairn's own.
    pub fn new(config: DevConfig, listener: Listener) -> Result<Self, ConfigError> {
        let refuse = |reason: &str| ConfigError {
            provider: config.name.to_string(),
            reason: reason.to_owned(),
        };
        if !listener.is_local() && !config.allow_off_loopback {
            return Err(refuse(
                "the dev provider serves only a loopback or unix socket listener \
                 unless its off-loopback override is set",
            ));
        }
        if let Some(token) = &config.token
            && (token.trim().is_empty() || token.starts_with(SECRET_PREFIX))
        {
            return Err(refuse(
                "the dev token is empty or starts with Cairn's prefix",
            ));
        }
        let login = Login {
            provider: config.name.clone(),
            subject: config.user.clone(),
            name: Some(config.user.clone()),
            verified_emails: config.verified_emails.clone(),
        };
        Ok(Self { config, login })
    }

    fn verdict(&self, request: Presented<'_>) -> Verdict {
        let presented = match &self.config.token {
            None => true,
            Some(token) => match request.bearer() {
                Some(bearer) if self.claims_bearer(bearer) => {
                    if !secrets_equal(bearer, token) {
                        return Verdict::Refused(Refusal::Credential);
                    }
                    true
                }
                _ => false,
            },
        };
        if !presented {
            Verdict::Absent
        } else if !self.config.allow_off_loopback && !request.peer.is_local() {
            Verdict::Refused(Refusal::Peer)
        } else {
            Verdict::Login(self.login.clone())
        }
    }
}

#[async_trait]
impl AuthProvider for DevProvider {
    fn name(&self) -> &Slug {
        &self.config.name
    }

    async fn authenticate(&self, request: Presented<'_>) -> Verdict {
        self.verdict(request)
    }

    fn claims_bearer(&self, token: &str) -> bool {
        self.config.token.is_some() && !token.starts_with(SECRET_PREFIX)
    }

    fn auto_link(&self) -> bool {
        self.config.auto_link
    }
}
