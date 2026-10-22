//! The browser host (ARCHITECTURE, Repository layout: `crates/wasm`; Web UI): the engine and
//! the service compiled to wasm with wasm-bindgen.
//!
//! For every host, the engine over JSON: a domain document read ([`read_document`]), derived
//! once ([`Derivation`]) and
//! projected ([`Projection`]), a draft patch applied locally to a journey ([`apply`]) or a
//! route ([`apply_route`]), a proposal previewed ([`preview`]), route files exported and imported ([`export_route`],
//! [`import_route`]), a route graph derived once ([`RouteDerivation`]) and projected as a journey
//! is (its level, its trace, its schedule in days from its start), and its notices
//! ([`route_notices`]). Every input and output is the schema's JSON, so each value is the
//! server's byte for byte.
//!
//! For the in-browser host, its composition root ([`BrowserRoot`]): the service over the
//! memory store seeded from the fixtures, the in-process notifier, and one local login,
//! with the API's reads, writes, and proposals.
//!
//! A failure is thrown to JavaScript as the JSON of a [`HostError`]. A panic aborts the
//! module (PRACTICES, Programmer errors panic); its message goes to the console first.

mod derivation;
mod error;
mod files;
pub mod fixtures;
mod local;
mod proposals;
mod reads;
mod root;
mod route;

#[cfg(feature = "server")]
pub mod cases;

pub use derivation::{Derivation, DraftRequest, Projection, read_document};
pub use error::HostError;
pub use files::{
    ExportRequest, ImportRequest, export_route, exported, import_route, imported, read_route_file,
    route_file_text,
};
pub use local::{
    AppliedLocally, ApplyRequest, PreviewRequest, RouteApplyRequest, apply, apply_locally,
    apply_route, apply_route_locally, preview, preview_locally,
};
pub use proposals::{
    ProposalAnswer, ProposalApply, ProposalCreate, ProposalEdit, ProposalListRequest,
    ProposalReview, ProposalStep, RelinkRequest, SaveAsRouteRequest, StaleBase, UpgradeRequest,
};
pub use reads::{
    Caller, IdentityReferences, JourneyIndexQuery, LinkedLogin, RouteDetail, RouteImport,
    RoutePage, RouteReference, RouteSummary, VersionJourneys,
};
pub use root::{
    BrowserRoot, HistoryAnswer, JourneyPage, JourneySummary, PatchAnswer, PatchRequest,
    RootSubscription, Taken, Tick,
};
pub use route::{
    RouteDerivation, RouteDeriveRequest, RouteNoticesRequest, route_notices, route_notices_of,
};

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use wasm_bindgen::prelude::wasm_bindgen;

// The crate version stays 0.0.0 (ARCHITECTURE, Build, run, deploy): wasm-bindgen hashes it into
// every import name of the generated glue, so a bump would churn web/wasm/generated/.
const _: () = assert!(
    matches!(env!("CARGO_PKG_VERSION").as_bytes(), b"0.0.0"),
    "the crate version stays 0.0.0: a release takes its version from its git tag at build time (CAIRN_VERSION), so do not bump Cargo.toml"
);

/// Runs a future the memory store drives to completion on its first poll: every service
/// operation over it, which awaits nothing else.
///
/// # Panics
///
/// When it is still pending: only a subscription's wait can pend, and nothing awaits one.
pub fn now_or_never<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("the memory store answers at once"),
    }
}

#[wasm_bindgen]
extern "C" {
    /// The console's `error`, where a panic's message goes before the module aborts.
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(message: &str);
}

/// Runs when the module is instantiated: a panic's message goes to the console, since the
/// abort that follows says only that the module trapped.
#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(|info| console_error(&info.to_string())));
}
