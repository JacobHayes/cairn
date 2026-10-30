//! The composition root types (ARCHITECTURE, Service layer and composition; Terms:
//! Composition root, Capabilities document): what the root fills in to assemble a
//! [`Service`](crate::Service). The root reads them from its
//! configuration.

use std::collections::BTreeSet;
use std::sync::Arc;

use cairn_schema::{Date, Deployment, DeriveInputs, IdentityKey, Slug, TimeZoneName, Timestamp};
use cairn_store::{Notifier, Store};
use jiff::civil::DateTime;
use jiff::tz::Offset;
use jiff::tz::TimeZone;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What a composition root assembles a service from.
pub struct Parts {
    /// The store, shared with the auth layer.
    pub store: Arc<dyn Store>,
    /// The notifier every commit is announced through.
    pub notifier: Arc<dyn Notifier>,
    /// The deployment's time zone.
    pub settings: DeploymentSettings,
    /// What the host offers.
    pub capabilities: Capabilities,
}

/// The deployment's settings that derive reads (ARCHITECTURE, Terms: Derive inputs): its
/// time zone (A9).
#[derive(Clone, Debug, PartialEq)]
pub struct DeploymentSettings {
    timezone: TimeZoneName,
    zone: TimeZone,
}

impl DeploymentSettings {
    /// The settings for a deployment in `timezone`, which the host has resolved to `zone`
    /// (the service reads no zone database, so the host brings its own: the system's on the
    /// server, a bundled one in the browser).
    ///
    /// # Panics
    ///
    /// When `zone` names an IANA zone other than `timezone`.
    #[must_use]
    pub fn new(timezone: TimeZoneName, zone: TimeZone) -> Self {
        assert!(
            zone.iana_name()
                .is_none_or(|name| name == timezone.as_str()),
            "the zone is the deployment's time zone"
        );
        Self { timezone, zone }
    }

    /// The deployment's time zone.
    #[must_use]
    pub fn timezone(&self) -> &TimeZoneName {
        &self.timezone
    }

    /// The UTC offset the deployment's zone has at the instant a time of day written with
    /// `offset` names, when it is not that offset: the one to write it with.
    #[must_use]
    pub fn offset_instead(&self, wall: DateTime, offset: Offset) -> Option<Offset> {
        let instant = offset.to_timestamp(wall).ok()?;
        Some(self.zone.to_offset(instant)).filter(|right| *right != offset)
    }

    /// A9: the calendar day `now` falls on in the deployment's time zone, the today every
    /// user of the deployment shares.
    #[must_use]
    pub fn today(&self, now: Timestamp) -> Date {
        now.to_zoned(self.zone.clone()).date()
    }

    /// The derive inputs for one call: `now` and its today, these settings, the viewer's
    /// identities, and the deployment context at its revision.
    #[must_use]
    pub fn derive_inputs(
        &self,
        now: Timestamp,
        viewer: BTreeSet<IdentityKey>,
        deployment: Deployment,
    ) -> DeriveInputs {
        DeriveInputs {
            today: self.today(now),
            now,
            timezone: self.timezone.clone(),
            viewer,
            deployment,
        }
    }
}

/// The capabilities document (`GET /api/capabilities`): what this host offers, so the UI shows
/// and hides features from it. Absence is the host's choice at its root, never a flag
/// checked inside an operation. The server always serves MCP and the revision stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    /// How people sign in, in the order the host asks.
    pub auth: Vec<AuthMethod>,
    /// Whether the in-app assistant is available.
    pub assistant: bool,
    /// How an agent reaches this host's MCP server.
    pub mcp: McpAccess,
    /// The release this host runs, as the binary reports it (a version, or `dev`).
    pub version: String,
}

// PRD: I2.
/// How an agent reaches this host's MCP server: what the Connect an agent page tells a
/// person. The host's root works it out from its configuration, so the UI reads facts and
/// never reasons about providers or builds an address.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct McpAccess {
    /// The MCP endpoint's absolute address: the public URL joined with the MCP path.
    pub url: String,
    /// How an agent signs in, in the order the providers are asked; empty when it cannot.
    pub sign_in: Vec<AgentSignIn>,
    /// Whether a person can make agent tokens here: some provider accepts them.
    pub tokens: bool,
}

impl McpAccess {
    /// The MCP server at `url` with no way for an agent to sign in and no tokens: what a host
    /// that configures no provider an agent can use offers.
    #[must_use]
    pub fn closed(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            sign_in: Vec::new(),
            tokens: false,
        }
    }
}

/// One way an agent signs in to the MCP server.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AgentSignIn {
    /// The built-in OAuth server: a browser step on first use; the agent is recorded beside
    /// the person.
    Oauth,
    /// Behind IAP, the relay to Google: a browser step with Google, the client signing in on
    /// the machine it runs on.
    GoogleOauth,
    /// Tailscale, either mode: no sign-in step, from a machine in the tailnet signed in as a
    /// person.
    Tailnet,
    /// The dev provider without a token: every request it accepts is its dev user.
    DevUser,
}

// PRD: H1.
/// One way to sign in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthMethod {
    /// The provider's configured name, as logins record it.
    pub name: Slug,
    /// What kind of provider it is.
    pub kind: AuthKind,
}

/// The kinds of sign-in a host can offer.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    /// A static token or named dev user, for local development.
    Dev,
    /// OAuth or OIDC against a registered provider.
    Oidc,
    /// The built-in OAuth server for MCP clients.
    BuiltinOauth,
    /// Tailscale login.
    Tailscale,
    /// Google Cloud Identity-Aware Proxy's signed assertion.
    GcpIap,
}
