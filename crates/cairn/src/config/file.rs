//! The configuration file as written (TOML), before the environment's overrides and before
//! validation. Every table refuses keys it does not know, so a misspelled key fails at
//! startup rather than being ignored. The values a deployment must choose (the database
//! path, the listen address, the public URL, the time zone) are optional here only so a
//! missing one is reported by name, beside every other problem, rather than as a parse
//! error.

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
    /// The auth providers, in the order a request is put to them.
    #[serde(default)]
    pub auth: Vec<ProviderFile>,
    /// The assistant's provider, when the deployment offers the assistant (I5).
    pub assistant: Option<AssistantFile>,
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
    /// Tailscale login, from the headers of the authenticating proxy in front.
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
    pub verified_emails: Vec<String>,
    pub token: Option<String>,
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
    /// The addresses or networks the authenticating proxy connects from.
    pub trusted_proxies: Vec<String>,
}

/// The Google Cloud IAP provider as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GcpIapFile {
    pub name: String,
    pub audience: String,
    /// The Google OAuth client MCP clients sign in through (the relay); absent, there is no
    /// relay.
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

/// The assistant's provider (ARCHITECTURE, Assistant).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AssistantFile {
    pub endpoint: String,
    pub model: String,
    pub api_key: Option<String>,
}
