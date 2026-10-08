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
}

/// The IAP provider.
pub struct GcpIapProvider {
    config: GcpIapConfig,
    clock: Clock,
    http: Outbound,
    key_set_url: JsonWebKeySetUrl,
    keys: Mutex<Option<Kept>>,
}

/// The key set as last fetched.
struct Kept {
    set: CoreJsonWebKeySet,
    fetched_at: Timestamp,
}

impl GcpIapProvider {
    /// The provider, verifying against Google's IAP keys.
    ///
    /// # Errors
    ///
    /// The audience is not an IAP resource, or the HTTP client cannot start.
    pub fn new(config: GcpIapConfig, clock: Clock) -> Result<Self, ConfigError> {
        let url = GOOGLE_KEY_SET_URL
            .parse()
            .map_err(|error: url::ParseError| ConfigError {
                provider: config.name.to_string(),
                reason: error.to_string(),
            })?;
        Self::with_key_set(config, clock, url)
    }

    /// The provider, verifying against the key set at `url`: Google's in a deployment, a
    /// stub's in tests (readiness ruling: every provider tests offline).
    ///
    /// # Errors
    ///
    /// The audience is not an IAP resource, the URL is neither https nor this machine, or
    /// the HTTP client cannot start.
    pub fn with_key_set(config: GcpIapConfig, clock: Clock, url: Url) -> Result<Self, ConfigError> {
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
        if !is_trusted_origin(&url) {
            return Err(refuse(format!(
                "the key set {url} is neither https nor this machine"
            )));
        }
        let http = Outbound::new().map_err(refuse)?;
        Ok(Self {
            config,
            clock,
            http,
            key_set_url: JsonWebKeySetUrl::from_url(url),
            keys: Mutex::new(None),
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

    fn authenticate<'a>(&'a self, request: Presented<'a>) -> BoxFuture<'a, Verdict> {
        let assertion = request
            .headers
            .get(ASSERTION_HEADER)
            .map(|value| value.to_str().map(str::trim));
        Box::pin(async move {
            match assertion {
                None => Verdict::Absent,
                Some(Ok(assertion)) if !assertion.is_empty() => self.verify(assertion).await,
                Some(_) => Verdict::Refused(Refusal::Credential),
            }
        })
    }

    fn auto_link(&self) -> bool {
        self.config.auto_link
    }
}

impl std::fmt::Debug for GcpIapProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GcpIapProvider")
            .field("config", &self.config)
            .field("key_set_url", &self.key_set_url.as_str())
            .finish_non_exhaustive()
    }
}
