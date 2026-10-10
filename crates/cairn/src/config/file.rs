//! The configuration file as written (TOML), before the environment's overrides and before
//! validation. Every table refuses keys it does not know, so a misspelled key fails at
//! startup rather than being ignored. The values a deployment must choose (the database
//! path, the listen address, the public URL, the time zone) are optional here only so a
//! missing one is reported by name, beside every other problem, rather than as a parse
//! error.

use cairn_assistant::Protocol;
use serde::Deserialize;

/// The whole file.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FileConfig {
    /// The database file's path; relative to the configuration file's directory.
    pub database: Option<String>,
    /// The address to listen on (`127.0.0.1:8080`).
    pub listen: Option<String>,
    /// The deployment's external base URL.
    pub public_url: Option<String>,
    /// The deployment's IANA time zone (A9).
    pub timezone: Option<String>,
    /// The rank constants (Priority): the PRD's defaults for any not given.
    #[serde(default)]
    pub rank: RankFile,
    /// The auth providers, in the order a request is put to them.
    #[serde(default)]
    pub auth: Vec<ProviderFile>,
    /// The assistant's provider, when the deployment offers the assistant (I5).
    pub assistant: Option<AssistantFile>,
}

/// The rank constants as written: plain numbers, each checked by validation.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub(crate) struct RankFile {
    pub urgency: f64,
    pub late: f64,
    pub gravity: f64,
    pub unlocks: f64,
    pub horizon_days: i64,
    pub undecided_discount: f64,
    pub other_owner_factor: f64,
}

impl Default for RankFile {
    /// The PRD's defaults (Priority), as the schema's `RankConstants::default` holds them.
    fn default() -> Self {
        let rank = cairn_schema::RankConstants::default();
        Self {
            urgency: rank.urgency.get(),
            late: rank.late.get(),
            gravity: rank.gravity.get(),
            unlocks: rank.unlocks.get(),
            horizon_days: i64::from(rank.horizon_days),
            undecided_discount: f64::from(rank.undecided_discount.get()) / 1_000.0,
            other_owner_factor: f64::from(rank.other_owner_factor.get()) / 1_000.0,
        }
    }
}

/// One auth provider (ARCHITECTURE, Auth), by kind.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ProviderFile {
    /// A static token or named dev user (H1).
    Dev(DevFile),
    /// Sign-in against an OIDC issuer (H1).
    Oidc(OidcFile),
    /// The built-in OAuth server for MCP clients and agent tokens (I2).
    BuiltinOauth(OAuthFile),
    /// Tailscale identity, direct or behind `tailscale serve`.
    Tailscale(TailscaleFile),
    /// Google Cloud Identity-Aware Proxy's signed assertion on each request.
    GcpIap(GcpIapFile),
}

impl ProviderFile {
    /// The provider's name as written.
    pub fn name(&self) -> &str {
        match self {
            ProviderFile::Dev(DevFile { name, .. })
            | ProviderFile::Oidc(OidcFile { name, .. })
            | ProviderFile::BuiltinOauth(OAuthFile { name, .. })
            | ProviderFile::Tailscale(TailscaleFile { name, .. })
            | ProviderFile::GcpIap(GcpIapFile { name, .. }) => name,
        }
    }
}

/// The dev provider as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DevFile {
    pub name: String,
    pub user: String,
    #[serde(default)]
    pub verified_emails: Vec<String>,
    pub token: Option<String>,
    #[serde(default)]
    pub auto_link: bool,
    #[serde(default)]
    pub allow_off_loopback: bool,
}

/// An OIDC provider as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OidcFile {
    pub name: String,
    pub issuer: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    #[serde(default)]
    pub auto_link: bool,
}

/// The built-in OAuth server as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OAuthFile {
    pub name: String,
    pub sign_in_with: Option<String>,
}

/// The Tailscale provider as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TailscaleFile {
    pub name: String,
    pub mode: TailscaleModeFile,
    pub socket: Option<String>,
    /// Proxy mode: the addresses or networks a proxy on another machine connects from;
    /// absent, the proxy is this machine's.
    pub trusted_proxies: Option<Vec<String>>,
    #[serde(default)]
    pub auto_link: bool,
}

/// The Google Cloud IAP provider as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GcpIapFile {
    pub name: String,
    pub audience: String,
    #[serde(default)]
    pub auto_link: bool,
}

/// Tailscale's two modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TailscaleModeFile {
    /// Ask tailscaled who the peer is.
    Direct,
    /// Trust an authenticating proxy's identity headers.
    Proxy,
}

/// The assistant's provider (ARCHITECTURE, Assistant).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AssistantFile {
    pub protocol: Protocol,
    pub endpoint: String,
    pub model: String,
    pub api_key: Option<String>,
}
