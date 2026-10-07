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
//! `wasm32-unknown-unknown` (rung 1 checks it) and the browser host runs the same service
//! over the memory store.

mod call;
mod compose;
mod consequence;
mod derived;
mod document;
mod drafting;
mod error;
mod load;
mod observe;
mod projections;
mod proposals;
mod reads;
mod routes;
mod viewer;
mod write;

pub use call::Call;
pub use compose::{AuthKind, AuthMethod, Capabilities, DeploymentSettings, Parts};
pub use drafting::ProposeError;
pub use error::ServiceError;
pub use observe::{DERIVE_DURATION, engine_call_active};
pub use projections::{ChildEntry, History, NodeDetail, Projected, ReadError};
pub use proposals::{ProposalReview, ProposalWritten, StaleBase};
pub use reads::RouteSummary;
pub use routes::PatchKeys;
pub use viewer::Viewer;
pub use write::{DomainPatch, NotADomainPatch, WriteError, Written};

use std::sync::Arc;

use cairn_store::{Notifier, Store};

/// The service over one store (ARCHITECTURE, Service layer and composition). Generic over the
/// store, whose trait is not object-safe (DECISIONS.md, 3.1); the notifier is a trait
/// object. Cloning shares the store and notifier.
pub struct Service<S> {
    store: Arc<S>,
    notifier: Arc<dyn Notifier>,
    settings: DeploymentSettings,
    capabilities: Capabilities,
    /// Recent derivations, shared by every clone (D6: invisible).
    memo: Arc<derived::Memo>,
}

impl<S> Clone for Service<S> {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
            notifier: Arc::clone(&self.notifier),
            settings: self.settings.clone(),
            capabilities: self.capabilities.clone(),
            memo: Arc::clone(&self.memo),
        }
    }
}

impl<S> std::fmt::Debug for Service<S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Service")
            .field("settings", &self.settings)
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}

impl<S: Store> Service<S> {
    /// Assembles the service from what the composition root picked.
    #[must_use]
    pub fn new(parts: Parts<S>) -> Self {
        Self {
            store: parts.store,
            notifier: parts.notifier,
            settings: parts.settings,
            capabilities: parts.capabilities,
            memo: Arc::default(),
        }
    }

    /// The capabilities document (`GET /capabilities`): what this host offers.
    #[must_use]
    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// The deployment's settings.
    #[must_use]
    pub fn settings(&self) -> &DeploymentSettings {
        &self.settings
    }
}
