//! The composition root types (ARCHITECTURE, Service layer and composition; Terms:
//! Composition root, Capabilities document): what each host's root fills in to assemble a
//! [`Service`](crate::Service). The server binary's root (4.7) reads them from its
//! configuration; the browser host's (4.5) fixes them.

use std::collections::BTreeSet;
use std::sync::Arc;

use cairn_schema::{
    Date, Deployment, DeriveInputs, EntityKey, RankConstants, Slug, TimeZoneName, Timestamp,
};
use cairn_store::Notifier;
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};

/// What a composition root assembles a service from.
pub struct Parts<S> {
    /// The store, shared with the auth layer.
    pub store: Arc<S>,
    /// The notifier every commit is announced through.
    pub notifier: Arc<dyn Notifier>,
    /// The deployment's time zone and rank constants.
    pub settings: DeploymentSettings,
    /// What the host offers.
    pub capabilities: Capabilities,
}

/// The deployment's settings that derive reads (ARCHITECTURE, Terms: Derive inputs): its
/// time zone (A9) and the rank constants (Priority).
#[derive(Clone, Debug, PartialEq)]
pub struct DeploymentSettings {
    timezone: TimeZoneName,
    zone: TimeZone,
    rank: RankConstants,
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
    pub fn new(timezone: TimeZoneName, zone: TimeZone, rank: RankConstants) -> Self {
        assert!(
            zone.iana_name()
                .is_none_or(|name| name == timezone.as_str()),
            "the zone is the deployment's time zone"
        );
        Self {
            timezone,
            zone,
            rank,
        }
    }

    /// The deployment's time zone.
    #[must_use]
    pub fn timezone(&self) -> &TimeZoneName {
        &self.timezone
    }

    /// The rank constants.
    #[must_use]
    pub fn rank(&self) -> RankConstants {
        self.rank
    }

    /// A9: the calendar day `now` falls on in the deployment's time zone, the today every
    /// user of the deployment shares.
    #[must_use]
    pub fn today(&self, now: Timestamp) -> Date {
        now.to_zoned(self.zone.clone()).date()
    }

    /// The derive inputs for one call: today, these settings, the viewer's entities, and
    /// the deployment context at its revision.
    #[must_use]
    pub fn derive_inputs(
        &self,
        today: Date,
        viewer: BTreeSet<EntityKey>,
        deployment: Deployment,
    ) -> DeriveInputs {
        DeriveInputs {
            today,
            timezone: self.timezone.clone(),
            rank: self.rank,
            viewer,
            deployment,
        }
    }
}

/// The capabilities document (`GET /api/capabilities`): what this host offers, so the UI shows
/// and hides features from it. Absence is the host's choice at its root, never a flag
/// checked inside an operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    /// How people sign in, in the order the host asks.
    pub auth: Vec<AuthMethod>,
    /// Whether the in-app assistant is available (I5: the UI is fully usable without it).
    pub assistant: bool,
    /// Whether the host serves MCP at `/api/mcp` (I2).
    pub mcp: bool,
    /// Whether the host streams revision ticks over SSE (H6).
    pub sse: bool,
}

impl Capabilities {
    /// The server's: MCP and SSE always (they are not optional on the server), the
    /// configured sign-in methods, and the assistant when one is configured.
    #[must_use]
    pub fn server(auth: Vec<AuthMethod>, assistant: bool) -> Self {
        Self {
            auth,
            assistant,
            mcp: true,
            sse: true,
        }
    }

    /// The browser host's: one local identity, and no assistant, MCP, or SSE (its views
    /// subscribe to the in-process notifier directly).
    #[must_use]
    pub fn browser() -> Self {
        let local = match "local".parse::<Slug>() {
            Ok(name) => name,
            Err(error) => unreachable!("`local` is a slug: {error:?}"),
        };
        Self {
            auth: vec![AuthMethod {
                name: local,
                kind: AuthKind::Local,
            }],
            assistant: false,
            mcp: false,
            sse: false,
        }
    }
}

/// One way to sign in (H1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthMethod {
    /// The provider's configured name, as identities record it.
    pub name: Slug,
    /// What kind of provider it is.
    pub kind: AuthKind,
}

/// The kinds of sign-in a host can offer (ARCHITECTURE, Auth).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    /// A static token or named dev user, for local development (H1).
    Dev,
    /// OAuth or OIDC against a registered provider (H1).
    Oidc,
    /// The built-in OAuth server for MCP clients (I2).
    BuiltinOauth,
    /// Tailscale identity.
    Tailscale,
    /// The browser host's single local identity.
    Local,
}
