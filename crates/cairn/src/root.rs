//! The composition root (ARCHITECTURE, Service layer and composition): one function that
//! builds the server from its configuration. The store is the caller's (SQLite at the
//! configured path for `cairn serve`, memory for `cairn demo`); the notifier is the
//! in-process one, whose ticks the API's stream coalesces on tokio's timer; the auth providers are the configured ones, in order; the assistant is present exactly
//! when configured, a `None` here and absent from the capabilities otherwise (I5). The API
//! (with MCP at `/api/mcp`), auth's routes, `/api/metrics`, and the embedded UI share one router
//! behind the `Host` allowlist, served on one port.

use std::fmt;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::HeaderValue;
use axum::middleware::map_response;
use axum::response::Response;
use cairn_assistant::Assistant;
use cairn_auth::{
    Accounts, Auth, AuthProvider, Clock, DevProvider, GcpIapProvider, OAuthServer, OidcProvider,
    TailscaleProvider,
};
use cairn_mcp::MCP_PATH;
use cairn_service::{AgentSignIn, AuthKind, AuthMethod, Capabilities, McpAccess, Parts, Service};
use cairn_store::{InProcessNotifier, Notifier, Store, StoreError};
use metrics_exporter_prometheus::PrometheusHandle;
use tokio::net::TcpListener;

use crate::assets::{self, Asset, WebBuildMissing};
use crate::config::{Config, Problem, Provider};
use crate::host::AllowedHosts;
use crate::{listener, telemetry};

/// What the root assembles a server from besides its configuration: the parts a test
/// replaces (the store, the notifier, the web build, the metrics recorder, the clock).
pub struct Assembly {
    /// The store, shared by the service, auth, and the assistant.
    pub store: Arc<dyn Store>,
    /// The notifier every commit is announced through.
    pub notifier: Arc<dyn Notifier>,
    /// The web build.
    pub assets: Vec<Asset>,
    /// The metrics `/api/metrics` serves.
    pub metrics: PrometheusHandle,
    /// The deployment clock.
    pub clock: Clock,
    /// The version the server reports (MCP `serverInfo`): the binary's [`crate::VERSION`];
    /// tests pass `dev`.
    pub version: &'static str,
}

/// Why the server did not start, or stopped.
#[derive(Debug)]
pub enum StartupError {
    /// An auth provider refused its configuration for this listener or URL.
    Providers(Vec<Problem>),
    /// The database could not be opened or migrated.
    Store(StoreError),
    /// The server has no web build.
    WebBuild(WebBuildMissing),
    /// The metrics recorder could not be installed.
    Metrics(String),
    /// The listen address could not be bound.
    Bind {
        /// The address.
        address: SocketAddr,
        /// Why.
        error: io::Error,
    },
    /// Serving failed.
    Serve(io::Error),
}

impl fmt::Display for StartupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StartupError::Providers(problems) => {
                write!(formatter, "the auth providers cannot start:")?;
                for problem in problems {
                    write!(formatter, "\n  {problem}")?;
                }
                Ok(())
            }
            StartupError::Store(error) => write!(formatter, "the database: {error}"),
            StartupError::WebBuild(missing) => write!(formatter, "{missing}"),
            StartupError::Metrics(error) => write!(formatter, "the metrics recorder: {error}"),
            StartupError::Bind { address, error } => {
                write!(formatter, "cannot listen on {address}: {error}")
            }
            StartupError::Serve(error) => write!(formatter, "serving: {error}"),
        }
    }
}

impl std::error::Error for StartupError {}

/// The capabilities the configuration offers: its sign-in methods in order, the assistant
/// when configured, the MCP access, and the version this build reports.
#[must_use]
pub fn capabilities(config: &Config) -> Capabilities {
    let auth = config
        .auth
        .iter()
        .map(|provider| {
            let (name, kind) = match provider {
                Provider::Dev(dev) => (&dev.name, AuthKind::Dev),
                Provider::Oidc(oidc) => (&oidc.name, AuthKind::Oidc),
                Provider::BuiltinOauth(oauth) => (&oauth.name, AuthKind::BuiltinOauth),
                Provider::Tailscale(tailscale) => (&tailscale.name, AuthKind::Tailscale),
                Provider::GcpIap(iap) => (&iap.name, AuthKind::GcpIap),
            };
            AuthMethod {
                name: name.clone(),
                kind,
            }
        })
        .collect();
    Capabilities {
        auth,
        assistant: config.assistant.is_some(),
        mcp: mcp_access(config),
        version: crate::VERSION.to_owned(),
    }
}

/// How an agent reaches the MCP server this configuration serves (I2): the public URL joined
/// with the MCP path, and each way the providers let an agent in, in the order they are asked.
/// A dev provider with a token, an OIDC provider, and an IAP provider without a relay add
/// nothing: an agent has no sign-in through them. Tokens are accepted only by the built-in
/// OAuth server.
fn mcp_access(config: &Config) -> McpAccess {
    let mut sign_in = Vec::new();
    let mut tokens = false;
    for provider in &config.auth {
        let how = match provider {
            Provider::BuiltinOauth(_) => {
                tokens = true;
                Some(AgentSignIn::Oauth)
            }
            Provider::GcpIap(iap) if iap.relay.is_some() => Some(AgentSignIn::GoogleOauth),
            Provider::Tailscale(_) => Some(AgentSignIn::Tailnet),
            Provider::Dev(dev) if dev.token.is_none() => Some(AgentSignIn::DevUser),
            Provider::Dev(_) | Provider::Oidc(_) | Provider::GcpIap(_) => None,
        };
        if let Some(how) = how
            && !sign_in.contains(&how)
        {
            sign_in.push(how);
        }
    }
    // The public URL's own path, if it had one, stays in front of the MCP path.
    let base = config.public_url.as_str().trim_end_matches('/');
    McpAccess {
        url: format!("{base}{MCP_PATH}"),
        sign_in,
        tokens,
    }
}

/// The configured auth providers over `accounts`, in order, each checked against the
/// listener and the public URL.
///
/// # Errors
///
/// Every provider that refuses its configuration (the dev provider off loopback without its
/// override, Tailscale's proxy mode on a listener others reach, an OIDC issuer over plain
/// http, an IAP audience that is not an IAP resource).
pub fn providers(
    config: &Config,
    accounts: &Accounts,
) -> Result<Vec<Arc<dyn AuthProvider>>, Vec<Problem>> {
    let base = &config.public_url;
    let mut built: Vec<Arc<dyn AuthProvider>> = Vec::with_capacity(config.auth.len());
    let mut problems = Vec::new();
    for provider in &config.auth {
        let made: Result<Arc<dyn AuthProvider>, _> = match provider {
            Provider::Dev(dev) => {
                DevProvider::new(dev.clone(), config.listen).map(|made| Arc::new(made) as _)
            }
            Provider::Oidc(oidc) => OidcProvider::new(oidc.clone(), accounts.clone(), base)
                .map(|made| Arc::new(made) as _),
            Provider::BuiltinOauth(oauth) => {
                Ok(Arc::new(OAuthServer::new(oauth.clone(), accounts.clone(), base)) as _)
            }
            Provider::Tailscale(tailscale) => {
                TailscaleProvider::new(tailscale.clone()).map(|made| Arc::new(made) as _)
            }
            Provider::GcpIap(iap) => {
                GcpIapProvider::new(iap.clone(), accounts.clock().clone(), base)
                    .map(|made| Arc::new(made) as _)
            }
        };
        match made {
            Ok(made) => built.push(made),
            Err(error) => problems.push(Problem::new(
                format!("auth.{}", error.provider),
                error.reason,
            )),
        }
    }
    if problems.is_empty() {
        Ok(built)
    } else {
        Err(problems)
    }
}

/// The whole server's router: the API with MCP, the assistant when configured, auth's
/// routes, `/api/metrics` behind the auth layer, and the UI, all behind the `Host` allowlist,
/// every response stamped with the web build's digest (ARCHITECTURE, Web UI: version skew).
///
/// # Errors
///
/// A provider refusing its configuration, or a web build with no page.
pub fn app(config: &Config, parts: Assembly) -> Result<Router, StartupError> {
    let service = Service::new(Parts {
        store: Arc::clone(&parts.store),
        notifier: parts.notifier,
        settings: config.settings(),
        capabilities: capabilities(config),
    });
    let accounts = Accounts::new(Arc::clone(&parts.store), parts.clock.clone());
    let providers = providers(config, &accounts).map_err(StartupError::Providers)?;
    let auth = Auth::new(accounts, &config.public_url, providers);
    let assistant = config.assistant.clone().map(|provider| {
        Assistant::new(
            service.clone(),
            Arc::clone(&parts.store),
            parts.clock.clone(),
            provider,
        )
    });
    let build = assets::digest(&parts.assets);
    let ui = assets::router(&parts.assets, &build).map_err(StartupError::WebBuild)?;
    let build = HeaderValue::try_from(build)
        .unwrap_or_else(|error| unreachable!("hex digits are a header value: {error}"));
    let metrics = auth.protect(telemetry::metrics_router(parts.metrics));
    // Metrics and the UI sit beside the API under its request limits: the in-flight limit
    // is per process, whatever the path. The UI answers every path the API does not keep.
    let app = cairn_api::router_beside(service, &auth, assistant, metrics, Some(ui), parts.version)
        .layer(map_response(move |mut response: Response| {
            response
                .headers_mut()
                .insert(assets::BUILD_HEADER, build.clone());
            std::future::ready(response)
        }));
    Ok(AllowedHosts::new(&config.public_url, config.listen).guard(app))
}

/// `cairn serve` and `cairn demo`: assembles the server over `store` with the embedded web
/// build, `beside` served with it (the demo's own routes; none for `serve`), and serves it on
/// the configured address until `shutdown` resolves.
///
/// # Errors
///
/// Any [`StartupError`].
pub async fn serve(
    config: &Config,
    store: Arc<dyn Store>,
    beside: Router,
    metrics: PrometheusHandle,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), StartupError> {
    let app = app(
        config,
        Assembly {
            store: Arc::clone(&store),
            notifier: Arc::new(InProcessNotifier::new()),
            assets: assets::load().map_err(StartupError::WebBuild)?,
            metrics,
            clock: Clock::system(),
            version: crate::VERSION,
        },
    )?
    .merge(beside);
    let listener = TcpListener::bind(config.listen)
        .await
        .map_err(|error| StartupError::Bind {
            address: config.listen,
            error,
        })?;
    let address = listener.local_addr().map_err(|error| StartupError::Bind {
        address: config.listen,
        error,
    })?;
    tracing::info!(
        %address,
        public_url = %config.public_url,
        database = %config.database.display(),
        "listening"
    );
    listener::warn_unsupported();
    // A store that has failed closed answers nothing more: the server stops, and exits
    // non-zero, so a supervisor restarts the process and the open recovers from disk.
    let closing = Arc::clone(&store);
    let stop = async move {
        tokio::select! {
            () = shutdown => {}
            () = failed_closed(closing.as_ref()) => {}
        }
    };
    let served = listener::serve(listener, app, stop)
        .await
        .map_err(StartupError::Serve);
    store.health().map_err(StartupError::Store)?;
    served
}

/// Resolves once `store` has failed closed. A health check reads the store's own state, so
/// looking once a second costs nothing.
async fn failed_closed(store: &dyn Store) {
    while store.health().is_ok() {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
