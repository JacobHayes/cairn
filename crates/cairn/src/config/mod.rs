//! The configuration (PRACTICES, Scope and dependencies: exactly the configuration surface):
//! the database file's path, the listen address, the public base URL, the auth providers
//! with their own security settings, the assistant's provider and credential, the
//! deployment's time zone, and the rank constants. One TOML file plus environment overrides
//! ([`env`]). The values a deployment must choose have no defaults, so a missing one fails
//! at startup naming it (ARCHITECTURE, Build, run, deploy); the rank constants default to
//! the PRD's (Priority: "the constants are defaults, configurable per deployment").
//!
//! Loading reports every problem it finds at once, each by the key or variable it is about,
//! rather than stopping at the first.

pub mod env;
mod file;
mod rank;
mod values;

use std::fmt;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use cairn_auth::{DevConfig, GcpIapConfig, Listener, OAuthConfig, OidcConfig, TailscaleConfig};
use cairn_schema::{RankConstants, TimeZoneName};
use cairn_service::DeploymentSettings;
use jiff::tz::TimeZone;
use url::Url;

pub use env::Environment;

/// A valid configuration.
#[derive(Clone, Debug)]
pub struct Config {
    /// The database file.
    pub database: PathBuf,
    /// The address the listener binds.
    pub listen: SocketAddr,
    /// The deployment's external base URL: links, OAuth redirects, MCP resource metadata,
    /// and the `Host` the listener serves.
    pub public_url: Url,
    /// The deployment's time zone (A9), with its rules from the system's zone database.
    pub timezone: TimeZoneName,
    /// The zone `timezone` names.
    pub zone: TimeZone,
    /// The rank constants (Priority).
    pub rank: RankConstants,
    /// The auth providers, in the order a request is put to them.
    pub auth: Vec<Provider>,
    /// The assistant's provider, when the deployment offers the assistant (I5).
    pub assistant: Option<cairn_assistant::protocol::ProviderConfig>,
}

impl Config {
    /// The deployment settings derive reads.
    #[must_use]
    pub fn settings(&self) -> DeploymentSettings {
        DeploymentSettings::new(self.timezone.clone(), self.zone.clone(), self.rank)
    }

    /// Where the listener accepts connections, as the auth providers judge it.
    #[must_use]
    pub fn listener(&self) -> Listener {
        Listener::Tcp(self.listen)
    }
}

/// One configured auth provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Provider {
    /// The dev provider.
    Dev(DevConfig),
    /// An OIDC issuer.
    Oidc(OidcConfig),
    /// The built-in OAuth server.
    BuiltinOauth(OAuthConfig),
    /// Tailscale.
    Tailscale(TailscaleConfig),
    /// Google Cloud IAP.
    GcpIap(GcpIapConfig),
}

/// One thing wrong with a configuration: the key or variable it is about, and what.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// The key (`timezone`, `auth.corp.issuer`, `rank`) or variable.
    pub key: String,
    /// What is wrong with it.
    pub message: String,
}

impl Problem {
    pub(crate) fn new(key: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.key, self.message)
    }
}

/// Every problem with a configuration, and where it was read from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problems {
    /// The file.
    pub source: PathBuf,
    /// What is wrong, in the order found. Never empty.
    pub problems: Vec<Problem>,
}

impl fmt::Display for Problems {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the configuration {} is not valid:",
            self.source.display()
        )?;
        for problem in &self.problems {
            write!(formatter, "\n  {problem}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Problems {}

/// Reads the configuration file at `path` with `environment`'s overrides.
///
/// # Errors
///
/// Every problem found: the file unreadable or not TOML, a value missing or malformed.
pub fn load(path: &Path, environment: &Environment) -> Result<Config, Problems> {
    let text = std::fs::read_to_string(path).map_err(|error| Problems {
        source: path.to_owned(),
        problems: vec![Problem::new("file", format!("cannot be read: {error}"))],
    })?;
    let directory = path.parent().unwrap_or(Path::new("."));
    parse(&text, directory, environment).map_err(|problems| Problems {
        source: path.to_owned(),
        problems,
    })
}

/// Parses a configuration's `text`, a relative database path in it being relative to
/// `directory`.
///
/// # Errors
///
/// Every problem found, never none.
///
/// # Panics
///
/// Never: a configuration that was not built always has a problem saying why.
pub fn parse(
    text: &str,
    directory: &Path,
    environment: &Environment,
) -> Result<Config, Vec<Problem>> {
    let mut file: file::FileConfig = toml::from_str(text).map_err(|error| {
        vec![Problem::new(
            "file",
            error.to_string().trim_end().to_owned(),
        )]
    })?;
    let (origin, mut problems) = env::apply(&mut file, environment);
    let built = values::build(file, origin, directory, &mut problems);
    match built {
        Some(config) if problems.is_empty() => Ok(config),
        _ => {
            assert!(!problems.is_empty(), "a configuration not built says why");
            Err(problems)
        }
    }
}

#[cfg(test)]
mod tests;
