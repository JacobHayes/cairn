//! The dev provider (H1: "a simple mode, static token or named dev user"). Local only: it
//! will not start on a listener other machines can reach unless its configuration says so
//! (the off-loopback override, a security choice about this provider, ARCHITECTURE Auth),
//! and without that override it refuses any request that is not from this machine.

use std::collections::BTreeSet;
use std::net::SocketAddr;

use async_trait::async_trait;
use cairn_schema::{Email, Login, Slug, Title};

use crate::error::{ConfigError, Refusal};
use crate::provider::{AuthProvider, Presented, Verdict, is_loopback};
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
    /// The dev provider for a server listening at `listen`.
    ///
    /// # Errors
    ///
    /// The listener is reachable from other machines and the override is off, or the token
    /// is empty.
    pub fn new(config: DevConfig, listen: SocketAddr) -> Result<Self, ConfigError> {
        let refuse = |reason: &str| ConfigError {
            provider: config.name.to_string(),
            reason: reason.to_owned(),
        };
        if !is_loopback(listen.ip()) && !config.allow_off_loopback {
            return Err(refuse(
                "the dev provider serves only a loopback listener \
                 unless its off-loopback override is set",
            ));
        }
        if config
            .token
            .as_ref()
            .is_some_and(|token| token.trim().is_empty())
        {
            return Err(refuse("the dev token is empty"));
        }
        if config
            .token
            .as_ref()
            .is_some_and(|token| token.starts_with(SECRET_PREFIX))
        {
            return Err(refuse(
                "the dev token cannot start with Cairn's secret prefix",
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
}

#[async_trait]
impl AuthProvider for DevProvider {
    async fn authenticate(&self, request: Presented<'_>) -> Verdict {
        if let Some(token) = &self.config.token {
            match request.bearer() {
                Some(bearer) if self.claims_bearer(bearer) => {
                    if !secrets_equal(bearer, token) {
                        return Verdict::Refused(Refusal::Credential);
                    }
                }
                _ => return Verdict::Absent,
            }
        }
        let local = request.peer.is_some_and(|peer| is_loopback(peer.ip()));
        if local || self.config.allow_off_loopback {
            Verdict::Login(self.login.clone())
        } else {
            Verdict::Refused(Refusal::Peer)
        }
    }

    fn claims_bearer(&self, token: &str) -> bool {
        self.config.token.is_some() && !token.starts_with(SECRET_PREFIX)
    }
}
