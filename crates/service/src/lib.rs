//! The service layer (ARCHITECTURE, Service layer and composition): the one caller of the
//! engine. Its operations are the shared vocabulary of the API, MCP, and the assistant, which
//! shape their own surfaces but never bypass it.
//!
//! A [`Service`] is assembled once per host by its composition root from [`Parts`]: a store,
//! a notifier, the deployment's settings (time zone and rank constants), and the
//! [`Capabilities`] the host offers. Every write takes a [`Call`]: who is acting (H2) and the
//! request's clock, from which today is computed once, in the deployment's time zone.
//!
//! Runtime-free: no tokio, database driver, or transport, so it builds for
//! `wasm32-unknown-unknown` (the lint step checks it), as the browser's engine module does.

mod call;
mod caller;
mod compose;
mod consequence;
mod derived;
mod document;
mod drafting;
mod error;
mod limits;
mod load;
mod notices;
mod observe;
mod projections;
mod proposals;
mod reads;
mod routes;
mod tour;
pub mod wire;
mod write;

pub use call::Call;
pub use caller::{Caller, UserName};
pub use compose::{
    AgentSignIn, AuthKind, AuthMethod, Capabilities, DeploymentSettings, McpAccess, Parts,
};
pub use drafting::ProposeError;
pub use error::ServiceError;
pub use limits::STALE_ATTEMPT_COUNT_MAX;
pub use observe::{DERIVE_DURATION, engine_call_active};
pub use projections::{ChildEntry, History, NodeDetail, Projected, ReadError};
pub use proposals::{ProposalListing, ProposalReview, ProposalWritten, StaleBase};
pub use reads::{IdentityReferences, RouteReference, RouteSummary};
pub use routes::PatchKeys;
pub use tour::TourView;
pub use write::{DomainPatch, NotADomainPatch, WriteError, Written};

use std::sync::Arc;

use cairn_store::{Notifier, Store, StoreError};

/// The service over one store (ARCHITECTURE, Service layer and composition). The store and
/// the notifier are trait objects, so the service and every layer above it are compiled once
/// whatever backend a host picks. Cloning shares the store and notifier.
#[derive(Clone)]
pub struct Service {
    store: Arc<dyn Store>,
    notifier: Arc<dyn Notifier>,
    settings: DeploymentSettings,
    capabilities: Capabilities,
    /// Recent derivations, shared by every clone (D6: invisible).
    memo: Arc<derived::Memo>,
}

impl std::fmt::Debug for Service {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Service")
            .field("settings", &self.settings)
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}

impl Service {
    /// Assembles the service from what the composition root picked.
    #[must_use]
    pub fn new(parts: Parts) -> Self {
        Self {
            store: parts.store,
            notifier: parts.notifier,
            settings: parts.settings,
            capabilities: parts.capabilities,
            memo: Arc::default(),
        }
    }

    /// The capabilities document (`GET /api/capabilities`): what this host offers.
    #[must_use]
    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// The deployment's settings.
    #[must_use]
    pub fn settings(&self) -> &DeploymentSettings {
        &self.settings
    }

    /// Whether the service can answer (`GET /healthz`): its store has not failed closed.
    /// It reads the store's own state, never its storage.
    ///
    /// # Errors
    ///
    /// The store's, once it has failed closed.
    pub fn health(&self) -> Result<(), StoreError> {
        self.store.health()
    }
}
