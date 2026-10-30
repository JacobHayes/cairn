//! The browser's engine (ARCHITECTURE, Repository layout: `crates/wasm`; Web UI): the pure
//! engine compiled to wasm with wasm-bindgen, for the functions the app calls locally.
//!
//! A domain document read ([`read_document`]), derived once ([`Derivation`]) and projected
//! ([`Projection`]), a draft patch applied locally to a journey ([`apply`]) or a route
//! ([`apply_route`]), a proposal previewed ([`preview`]), route files read and written
//! ([`read_route_file`], [`route_file_text`]), a route graph derived once ([`RouteDerivation`]) and
//! projected as a journey is (its level, its trace, its schedule in days from its start), and
//! its notices ([`route_notices`]). Every input and output is the schema's JSON, so each value
//! is the server's byte for byte.
//!
//! A failure is thrown to JavaScript as the JSON of a [`HostError`]. A panic aborts the
//! module (PRACTICES, Programmer errors panic); its message goes to the console first.

mod derivation;
mod error;
mod files;
mod local;
mod route;

pub use derivation::{Derivation, DraftRequest, Projection, read_document};
pub use error::HostError;
pub use files::{read_route_file, route_file_text};
pub use local::{
    AppliedLocally, ApplyRequest, PreviewRequest, RouteApplyRequest, apply, apply_locally,
    apply_route, apply_route_locally, condition_problem, condition_problem_of, draft_stored,
    draft_written, insertion_plan, node_form_problems_text, preview, preview_locally, removal_plan,
    value_problems_text,
};
pub use route::{
    RouteDerivation, RouteDeriveRequest, RouteNoticesRequest, route_notices, route_notices_of,
};

use wasm_bindgen::prelude::wasm_bindgen;

// The crate version stays 0.0.0 (ARCHITECTURE, Build, run, deploy): wasm-bindgen hashes it into
// every import name of the generated glue, so a bump would churn web/wasm/generated/.
const _: () = assert!(
    matches!(env!("CARGO_PKG_VERSION").as_bytes(), b"0.0.0"),
    "the crate version stays 0.0.0: a release takes its version from its git tag at build time (CAIRN_VERSION), so do not bump Cargo.toml"
);

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
