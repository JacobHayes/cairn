//! The Google Cloud IAP provider (H1; ARCHITECTURE, Auth): Cairn behind Identity-Aware
//! Proxy, which signs every request it lets through with a JWT in the
//! `x-goog-iap-jwt-assertion` header. Each request's assertion is verified (signature
//! against Google's published IAP keys, issuer, the configured audience, expiry) and names
//! its user; nothing is trusted because of where the request came from, so the provider
//! runs on any listener (decisions/2026-10-08-behind-iap-cairn-verifies-the-signed-assertion.md).
//!
//! An assertion has the shape of an ID token (`iss`, `aud`, `sub`, `exp`, `iat`, `email`),
//! so `openidconnect`'s verifier checks it, as it checks the OIDC provider's ID tokens;
//! nothing here validates a token by hand. IAP signs with ES256 only. Beyond what the
//! verifier checks, an assertion may last no longer than IAP's own do, with the skew at
//! either end.
//!
//! The key set is fetched on first use and kept for at most `IAP_KEY_SET_AGE_MAX`, so a key
//! Google withdraws stops being trusted at the next fetch. A token naming a key the kept set
//! lacks (Google rotated its keys) fetches it again too. Any try, failed or not, holds off
//! the next for `IAP_KEY_SET_REFETCH_INTERVAL_MIN`, so neither a stream of forged key ids
//! nor an outage at Google makes Cairn fetch on every request. One fetch runs at a time, on
//! its own task so a request dropped meanwhile cannot cut it short, and the requests that
//! need it wait for it, at most an outbound call; a request whose key the kept set holds,
//! while the set is within its age, never waits, and no lock is held across the call.
//!
//! Given the Google OAuth client IAP's programmatic access allows, the provider also relays
//! an MCP client's sign-in to Google (`relay`), so the bearer the client ends up holding is
//! a Google ID token IAP accepts. Such a bearer counts only beside a valid assertion on the
//! same request, which names the user; Cairn never reads login from the bearer itself
//! (decisions/2026-10-16-mcp-clients-behind-iap-sign-in-through-a-relay-to-google.md).

mod relay;

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use axum::Router;
use cairn_schema::{Email, Login, Slug, Timestamp, Title};
use openidconnect::core::{
    CoreIdToken, CoreIdTokenClaims, CoreIdTokenVerifier, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
};
use openidconnect::{
    ClaimsVerificationError, ClientId, IssuerUrl, JsonWebKeySetUrl, Nonce,
    SignatureVerificationError,
};
use url::Url;

use crate::clock::Clock;
use crate::error::{ConfigError, Refusal};
use crate::layer::Authenticate;
use crate::limits::{
    IAP_ASSERTION_CLOCK_SKEW_MAX, IAP_ASSERTION_LIFETIME_MAX, IAP_KEY_SET_AGE_MAX,
    IAP_KEY_SET_REFETCH_INTERVAL_MIN,
};
use crate::oidc::chrono_time;
use crate::outbound::{Outbound, is_trusted_origin};
use crate::provider::{AuthProvider, Presented, Verdict};

/// The header IAP puts its signed assertion in.
pub const ASSERTION_HEADER: &str = "x-goog-iap-jwt-assertion";
/// The issuer of every IAP assertion.
const ISSUER: &str = "https://cloud.google.com/iap";
/// How IAP's subject for a Google account begins. An Identity Platform (external) identity's
/// begins with its token issuer and tenant instead, and its email is that provider's,
/// which Google has not verified.
const GOOGLE_ACCOUNT_SUBJECT_PREFIX: &str = "accounts.google.com:";
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
    keys: Arc<Mutex<Keys>>,
    /// Held across a fetch of the key set, so one runs at a time.
    fetching: Arc<tokio::sync::Mutex<()>>,
    relay: Option<Arc<relay::Relay>>,
}

/// The key set as last fetched, and when a fetch was last tried.
#[derive(Default)]
struct Keys {
    kept: Option<Kept>,
    tried_at: Option<Timestamp>,
}

/// The key set as last fetched.
struct Kept {
    set: Arc<CoreJsonWebKeySet>,
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
            keys: Arc::new(Mutex::new(Keys::default())),
            fetching: Arc::new(tokio::sync::Mutex::new(())),
            relay,
            config,
        })
    }

    /// The login a presented assertion names, or why it is refused.
    async fn verify(&self, assertion: &str) -> Verdict {
        let Ok(token) = assertion.parse::<CoreIdToken>() else {
            return Verdict::Refused(Refusal::Credential);
        };
        if let Some(set) = self.current_keys() {
            let checked = self.check(&token, &set);
            let unknown_key = matches!(
                checked,
                Err(ClaimsVerificationError::SignatureVerification(
                    SignatureVerificationError::NoMatchingKey
                ))
            );
            if !unknown_key {
                return self.verdict(checked);
            }
        }
        match self.refresh().await {
            Ok(set) => self.verdict(self.check(&token, &set)),
            Err(refusal) => Verdict::Refused(refusal),
        }
    }

    /// The verdict on checked claims.
    fn verdict(&self, checked: Result<CoreIdTokenClaims, ClaimsVerificationError>) -> Verdict {
        match checked {
            Ok(claims) => self.login(&claims),
            Err(_) => Verdict::Refused(Refusal::Credential),
        }
    }

    /// The kept key set, unless it is missing or older than `IAP_KEY_SET_AGE_MAX`.
    fn current_keys(&self) -> Option<Arc<CoreJsonWebKeySet>> {
        let keys = lock(&self.keys);
        let kept = keys.kept.as_ref()?;
        self.within(kept.fetched_at, IAP_KEY_SET_AGE_MAX)
            .then(|| Arc::clone(&kept.set))
    }

    /// The key set fetched now. When a fetch was tried within the refetch interval (most
    /// often the one this request waited behind), none is made: the answer is the current
    /// kept set, or unavailable without one. The try is recorded before the call, so a
    /// failed one holds off the next as a good one does.
    async fn refresh(&self) -> Result<Arc<CoreJsonWebKeySet>, Refusal> {
        let fetching = Arc::clone(&self.fetching).lock_owned().await;
        let now = self.clock.now();
        let tried_lately = {
            let mut keys = lock(&self.keys);
            let lately = keys
                .tried_at
                .is_some_and(|tried| self.within(tried, IAP_KEY_SET_REFETCH_INTERVAL_MIN));
            if !lately {
                keys.tried_at = Some(now);
            }
            lately
        };
        if tried_lately {
            return self.current_keys().ok_or_else(|| {
                Refusal::Unavailable("the IAP key set: the last fetch failed".to_owned())
            });
        }
        // The fetch keeps what it gets and lets the next one start itself, so a request
        // dropped mid-fetch cannot leave the try recorded and nothing kept.
        let (keys, url, http) = (
            Arc::clone(&self.keys),
            self.key_set_url.clone(),
            self.http.clone(),
        );
        let fetch = tokio::spawn(async move {
            let fetched = CoreJsonWebKeySet::fetch_async(&url, &http).await;
            let set = Arc::new(fetched.map_err(|error| error.to_string())?);
            lock(&keys).kept = Some(Kept {
                set: Arc::clone(&set),
                fetched_at: now,
            });
            drop(fetching);
            Ok::<_, String>(set)
        });
        match fetch.await {
            Ok(Ok(set)) => Ok(set),
            Ok(Err(error)) => Err(format!("the IAP key set: {error}")),
            Err(error) => Err(format!("the IAP key set fetch: {error}")),
        }
        .map_err(Refusal::Unavailable)
    }

    /// Whether `since` is less than `interval` ago. A `since` ahead of now (the clock
    /// stepped back) is not within, so a step back neither holds off fetches nor keeps a
    /// set trusted for as long as the step.
    fn within(&self, since: Timestamp, interval: std::time::Duration) -> bool {
        let interval =
            jiff::SignedDuration::try_from(interval).unwrap_or(jiff::SignedDuration::MAX);
        let elapsed = self.clock.now().duration_since(since);
        !elapsed.is_negative() && elapsed < interval
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
        let claims = token.claims(&verifier, no_nonce).cloned()?;
        let lifetime = claims.expiration() - claims.issue_time();
        let lifetime_max = IAP_ASSERTION_LIFETIME_MAX + 2 * IAP_ASSERTION_CLOCK_SKEW_MAX;
        if lifetime
            .to_std()
            .map_or(true, |lifetime| lifetime > lifetime_max)
        {
            return Err(ClaimsVerificationError::Expired(format!(
                "lasts {lifetime}, past IAP's {lifetime_max:?}"
            )));
        }
        Ok(claims)
    }

    /// The login verified claims name: the subject is IAP's stable account id, and a
    /// Google account's email is Google's, verified by Google (H3). An external identity's
    /// email is not listed as verified, so it never matches a user by email and, with no
    /// verified email, the login is refused.
    fn login(&self, claims: &CoreIdTokenClaims) -> Verdict {
        let Ok(subject) = claims.subject().as_str().parse::<Title>() else {
            return Verdict::Refused(Refusal::Credential);
        };
        let google_account = subject.as_str().starts_with(GOOGLE_ACCOUNT_SUBJECT_PREFIX);
        let email = claims
            .email()
            .map(|email| email.as_str())
            .filter(|_| google_account);
        let verified_emails = email
            .and_then(|email| email.parse::<Email>().ok())
            .into_iter()
            .collect();
        Verdict::Login(Login {
            provider: self.config.name.clone(),
            subject,
            name: None,
            verified_emails,
        })
    }
}

#[async_trait]
impl AuthProvider for GcpIapProvider {
    /// H1, I2: the request's assertion names its user. A Google ID token as the bearer
    /// (the relay's) counts only beside a valid assertion: alone, it is refused rather than
    /// left for another provider, and it is never read for who the user is.
    async fn authenticate(&self, request: Presented<'_>) -> Verdict {
        let mut assertions = request.headers.get_all(ASSERTION_HEADER).iter();
        let assertion = assertions.next().map(|value| value.to_str().map(str::trim));
        let repeated = assertions.next().is_some();
        let google_bearer = request
            .bearer()
            .is_some_and(|token| self.claims_bearer(token));
        match assertion {
            // IAP sends one; which of several to believe is not Cairn's to guess.
            _ if repeated => Verdict::Refused(Refusal::Credential),
            None if google_bearer => Verdict::Refused(Refusal::Credential),
            None => Verdict::Absent,
            Some(Ok(assertion)) if !assertion.is_empty() => self.verify(assertion).await,
            Some(_) => Verdict::Refused(Refusal::Credential),
        }
    }

    /// With the relay on, a bearer shaped as an ID token is the relay's Google ID token,
    /// to be weighed by `authenticate`. This classifies; it checks nothing.
    fn claims_bearer(&self, token: &str) -> bool {
        self.relay.is_some() && relay::is_id_token(token)
    }

    /// The relay's OAuth endpoints, when it has a Google client.
    fn router(&self, _authenticator: Arc<dyn Authenticate>) -> Option<Router> {
        self.relay.as_ref().map(relay::router)
    }
}

/// The key state; a panic elsewhere while holding it leaves nothing half-written.
fn lock(keys: &Mutex<Keys>) -> MutexGuard<'_, Keys> {
    keys.lock().unwrap_or_else(PoisonError::into_inner)
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
