//! The Google Cloud IAP provider (H1; ARCHITECTURE, Auth): Cairn behind Identity-Aware
//! Proxy, which signs every request it lets through with a JWT in the
//! `x-goog-iap-jwt-assertion` header. Each request's assertion is verified (signature
//! against Google's published IAP keys, issuer, the configured audience, expiry) and names
//! its user; nothing is trusted because of where the request came from, so the provider
//! runs on any listener (decisions/2026-10-08-behind-iap-cairn-verifies-the-signed-assertion.md).
//!
//! An assertion has the shape of an ID token (`iss`, `aud`, `sub`, `exp`, `iat`, `email`),
//! so `openidconnect`'s verifier checks it, as it checks the OIDC provider's ID tokens;
//! nothing here validates a token by hand. IAP signs with ES256 only.
//!
//! The key set is fetched on first use and kept. A token naming a key the kept set lacks
//! (Google rotated its keys) fetches it again, at most once per
//! `IAP_KEY_SET_REFETCH_INTERVAL_MIN`, so a stream of forged key ids cannot make Cairn
//! fetch on every request.
//!
//! Given the Google OAuth client IAP's programmatic access allows, the provider also relays
//! an MCP client's sign-in to Google (`relay`), so the bearer the client ends up holding is
//! a Google ID token IAP accepts. Such a bearer counts only beside a valid assertion on the
//! same request, which names the user; Cairn never reads identity from the bearer itself
//! (decisions/2026-10-16-mcp-clients-behind-iap-sign-in-through-a-relay-to-google.md).

mod relay;

use std::sync::Arc;

use axum::Router;
use cairn_schema::{Email, Identity, Slug, Timestamp, Title};
use openidconnect::core::{
    CoreIdToken, CoreIdTokenClaims, CoreIdTokenVerifier, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
};
use openidconnect::{
    ClaimsVerificationError, ClientId, IssuerUrl, JsonWebKeySetUrl, Nonce,
    SignatureVerificationError,
};
use tokio::sync::Mutex;
use url::Url;

use crate::clock::Clock;
use crate::error::{ConfigError, Refusal};
use crate::layer::Authenticate;
use crate::limits::{IAP_ASSERTION_CLOCK_SKEW_MAX, IAP_KEY_SET_REFETCH_INTERVAL_MIN};
use crate::oidc::chrono_time;
use crate::outbound::{Outbound, is_trusted_origin};
use crate::provider::{AuthProvider, BoxFuture, Presented, Verdict};

/// The header IAP puts its signed assertion in.
pub const ASSERTION_HEADER: &str = "x-goog-iap-jwt-assertion";
/// The issuer of every IAP assertion.
const ISSUER: &str = "https://cloud.google.com/iap";
/// Where Google publishes the keys IAP signs with, as a JSON Web Key set.
pub const GOOGLE_KEY_SET_URL: &str = "https://www.gstatic.com/iap/verify/public_key-jwk";
/// Google's authorization endpoint, where the relay sends a browser to sign in.
pub const GOOGLE_AUTHORIZATION_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
/// Google's token endpoint, where the relay exchanges a code or a refresh token.
pub const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

/// The IAP provider's configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GcpIapConfig {
    /// The provider's name.
    pub name: Slug,
    /// The audience IAP signs for: the protected resource,
    /// `/projects/<project number>/global/backendServices/<backend service id>` behind a
    /// load balancer.
    pub audience: String,
    /// H3: link a new identity to the user holding one of its verified emails.
    pub auto_link: bool,
    /// I2: the Google OAuth client on IAP's programmatic allowlist, through which MCP
    /// clients sign in; without it the provider serves no OAuth endpoints.
    pub relay: Option<RelayClient>,
}

/// The Google OAuth client the relay signs MCP clients in with. Its secret never leaves
/// the server, and `Debug` does not print it.
#[derive(Clone, PartialEq, Eq)]
pub struct RelayClient {
    /// The client's id at Google: the audience of the ID tokens IAP accepts.
    pub client_id: String,
    /// The client's secret at Google.
    pub client_secret: String,
}

impl std::fmt::Debug for RelayClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RelayClient")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .finish()
    }
}

/// The Google endpoints the provider calls: fixed in a deployment, stubs in tests
/// (readiness ruling: every provider tests offline). Nothing a request carries changes
/// them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoogleEndpoints {
    /// The key set IAP signs with.
    pub iap_key_set: Url,
    /// Google's authorization endpoint.
    pub authorization: Url,
    /// Google's token endpoint.
    pub token: Url,
}

impl GoogleEndpoints {
    /// Google's own.
    ///
    /// # Panics
    ///
    /// Never: the constants are URLs.
    #[must_use]
    pub fn google() -> Self {
        let parse = |url: &str| match Url::parse(url) {
            Ok(url) => url,
            Err(error) => unreachable!("{url} is a URL: {error}"),
        };
        Self {
            iap_key_set: parse(GOOGLE_KEY_SET_URL),
            authorization: parse(GOOGLE_AUTHORIZATION_URL),
            token: parse(GOOGLE_TOKEN_URL),
        }
    }
}

/// The IAP provider.
pub struct GcpIapProvider {
    config: GcpIapConfig,
    clock: Clock,
    http: Outbound,
    key_set_url: JsonWebKeySetUrl,
    keys: Mutex<Option<Kept>>,
    relay: Option<Arc<relay::Relay>>,
}

/// The key set as last fetched.
struct Kept {
    set: CoreJsonWebKeySet,
    fetched_at: Timestamp,
}

impl GcpIapProvider {
    /// The provider for a deployment at the public `base` URL, calling Google.
    ///
    /// # Errors
    ///
    /// As [`GcpIapProvider::with_endpoints`].
    pub fn new(config: GcpIapConfig, clock: Clock, base: &Url) -> Result<Self, ConfigError> {
        Self::with_endpoints(config, clock, base, GoogleEndpoints::google())
    }

    /// The provider, calling `endpoints`: Google's in a deployment, stubs' in tests.
    ///
    /// # Errors
    ///
    /// The audience is not an IAP resource, an endpoint is neither https nor this machine,
    /// the relay's client id or secret is empty or padded, or the HTTP client cannot start.
    pub fn with_endpoints(
        config: GcpIapConfig,
        clock: Clock,
        base: &Url,
        endpoints: GoogleEndpoints,
    ) -> Result<Self, ConfigError> {
        let refuse = |reason: String| ConfigError {
            provider: config.name.to_string(),
            reason,
        };
        if !config.audience.starts_with("/projects/") || config.audience.trim() != config.audience {
            return Err(refuse(format!(
                "the audience {:?} is not an IAP resource: \
                 /projects/<number>/global/backendServices/<id>",
                config.audience
            )));
        }
        for url in [
            &endpoints.iap_key_set,
            &endpoints.authorization,
            &endpoints.token,
        ] {
            if !is_trusted_origin(url) {
                return Err(refuse(format!(
                    "the Google endpoint {url} is neither https nor this machine"
                )));
            }
        }
        let http = Outbound::new().map_err(refuse)?;
        let relay = match &config.relay {
            Some(client) => {
                let made = relay::Relay::new(&config.name, client, base, &endpoints, http.clone());
                Some(Arc::new(made.map_err(refuse)?))
            }
            None => None,
        };
        Ok(Self {
            clock,
            http,
            key_set_url: JsonWebKeySetUrl::from_url(endpoints.iap_key_set),
            keys: Mutex::new(None),
            relay,
            config,
        })
    }

    /// The identity a presented assertion names, or why it is refused.
    async fn verify(&self, assertion: &str) -> Verdict {
        let Ok(token) = assertion.parse::<CoreIdToken>() else {
            return Verdict::Refused(Refusal::Credential);
        };
        let mut kept = self.keys.lock().await;
        if kept.is_none() {
            match self.fetch().await {
                Ok(fetched) => *kept = Some(fetched),
                Err(refusal) => return Verdict::Refused(refusal),
            }
        }
        let Some(current) = kept.as_ref() else {
            return Verdict::Refused(Refusal::Unavailable("no IAP key set".to_owned()));
        };
        let checked = self.check(&token, &current.set);
        let unknown_key = matches!(
            checked,
            Err(ClaimsVerificationError::SignatureVerification(
                SignatureVerificationError::NoMatchingKey
            ))
        );
        let checked = if unknown_key && self.may_refetch(current.fetched_at) {
            match self.fetch().await {
                Ok(fetched) => {
                    let checked = self.check(&token, &fetched.set);
                    *kept = Some(fetched);
                    checked
                }
                Err(refusal) => return Verdict::Refused(refusal),
            }
        } else {
            checked
        };
        drop(kept);
        match checked {
            Ok(claims) => self.identity(&claims),
            Err(_) => Verdict::Refused(Refusal::Credential),
        }
    }

    /// The assertion's claims, if `keys` verify it and it is for this audience and current.
    /// IAP sets no nonce.
    fn check(
        &self,
        token: &CoreIdToken,
        keys: &CoreJsonWebKeySet,
    ) -> Result<CoreIdTokenClaims, ClaimsVerificationError> {
        let skew = jiff::SignedDuration::try_from(IAP_ASSERTION_CLOCK_SKEW_MAX)
            .unwrap_or(jiff::SignedDuration::ZERO);
        let now = self.clock.now();
        let latest_issue = chrono_time(now.checked_add(skew).unwrap_or(now));
        // Read `skew` earlier than now, so an assertion that expired within the skew passes.
        let read = now.checked_sub(skew).unwrap_or(now);
        let verifier = CoreIdTokenVerifier::new_public_client(
            ClientId::new(self.config.audience.clone()),
            IssuerUrl::new(ISSUER.to_owned()).map_err(|error| {
                ClaimsVerificationError::Other(format!("the IAP issuer: {error}"))
            })?,
            keys.clone(),
        )
        .set_allowed_algs([CoreJwsSigningAlgorithm::EcdsaP256Sha256])
        .set_time_fn(move || chrono_time(read))
        .set_issue_time_verifier_fn(move |issued| {
            if issued > latest_issue {
                Err(format!("issued in the future, at {issued}"))
            } else {
                Ok(())
            }
        });
        let no_nonce = |_: Option<&Nonce>| Ok(());
        token.claims(&verifier, no_nonce).cloned()
    }

    /// Whether a token naming an unknown key may fetch the key set again: the last fetch
    /// was at least the refetch interval ago.
    fn may_refetch(&self, fetched_at: Timestamp) -> bool {
        let interval = jiff::SignedDuration::try_from(IAP_KEY_SET_REFETCH_INTERVAL_MIN)
            .unwrap_or(jiff::SignedDuration::MAX);
        let since = self.clock.now().duration_since(fetched_at);
        since >= interval
    }

    /// Google's key set, now.
    async fn fetch(&self) -> Result<Kept, Refusal> {
        let set = CoreJsonWebKeySet::fetch_async(&self.key_set_url, &self.http)
            .await
            .map_err(|error| Refusal::Unavailable(format!("the IAP key set: {error}")))?;
        Ok(Kept {
            set,
            fetched_at: self.clock.now(),
        })
    }

    /// The identity verified claims name: the subject is IAP's stable account id, and the
    /// email is Google's, verified by Google (H3).
    fn identity(&self, claims: &CoreIdTokenClaims) -> Verdict {
        let Ok(subject) = claims.subject().as_str().parse::<Title>() else {
            return Verdict::Refused(Refusal::Credential);
        };
        let email = claims.email().map(|email| email.as_str());
        let display = email
            .and_then(|email| email.parse::<Title>().ok())
            .unwrap_or_else(|| subject.clone());
        let verified_emails = email
            .and_then(|email| email.parse::<Email>().ok())
            .into_iter()
            .collect();
        Verdict::Identity(Identity {
            provider: self.config.name.clone(),
            subject,
            display,
            verified_emails,
        })
    }
}

impl AuthProvider for GcpIapProvider {
    fn name(&self) -> &Slug {
        &self.config.name
    }

    /// H1, I2: the request's assertion names its user. A Google ID token as the bearer
    /// (the relay's) counts only beside a valid assertion: alone, it is refused rather than
    /// left for another provider, and it is never read for who the user is.
    fn authenticate<'a>(&'a self, request: Presented<'a>) -> BoxFuture<'a, Verdict> {
        let assertion = request
            .headers
            .get(ASSERTION_HEADER)
            .map(|value| value.to_str().map(str::trim));
        let google_bearer = request
            .bearer()
            .is_some_and(|token| self.claims_bearer(token));
        Box::pin(async move {
            match assertion {
                None if google_bearer => Verdict::Refused(Refusal::Credential),
                None => Verdict::Absent,
                Some(Ok(assertion)) if !assertion.is_empty() => self.verify(assertion).await,
                Some(_) => Verdict::Refused(Refusal::Credential),
            }
        })
    }

    /// With the relay on, a bearer shaped as an ID token is the relay's Google ID token,
    /// to be weighed by `authenticate`. This classifies; it checks nothing.
    fn claims_bearer(&self, token: &str) -> bool {
        self.relay.is_some() && relay::is_id_token(token)
    }

    fn auto_link(&self) -> bool {
        self.config.auto_link
    }

    /// The relay's OAuth endpoints, when it has a Google client.
    fn router(&self, authenticator: Arc<dyn Authenticate>) -> Option<Router> {
        let _ = authenticator;
        self.relay.as_ref().map(relay::router)
    }
}

impl std::fmt::Debug for GcpIapProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GcpIapProvider")
            .field("config", &self.config)
            .field("key_set_url", &self.key_set_url.as_str())
            .field("relay", &self.relay.is_some())
            .finish_non_exhaustive()
    }
}
