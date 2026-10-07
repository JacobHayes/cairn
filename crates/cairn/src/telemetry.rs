//! Structured logs and metrics (ARCHITECTURE, Observability; PRD Non-functional: Observable),
//! and the panic policy (PRACTICES, Errors, panics, and rejections).
//!
//! Logs are JSON lines on standard error, at info and above, each carrying the span it was
//! written in (a request's id, method, and endpoint; a patch's id). Metrics go through the
//! `metrics` facade to a Prometheus recorder, served at `/metrics` behind the auth layer: the
//! API's request counts and latencies by endpoint, MCP tool calls by tool, patch outcomes,
//! caught engine panics, derive durations, and store commit durations.

use std::io::Write;
use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::response::IntoResponse;
use axum::routing::get;
use metrics_exporter_prometheus::{BuildError, PrometheusBuilder, PrometheusHandle};

use crate::limits::METRICS_UPKEEP_INTERVAL;

/// Installs the JSON log subscriber for the process; a second call does nothing.
pub fn init_logs() {
    let installed = tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::INFO)
        .with_current_span(true)
        .with_span_list(false)
        .with_writer(std::io::stderr)
        // A log that cannot be written (standard error closed under a supervisor) is lost,
        // never a panic, which would stop the process (PRACTICES: panics outside the engine
        // abort).
        .log_internal_errors(false)
        .try_init();
    // Only a second call fails, and the first subscriber stands.
    drop(installed);
}

/// Installs the process's metrics recorder and starts its upkeep on the current runtime.
///
/// # Errors
///
/// When a recorder is already installed.
pub fn install_metrics() -> Result<PrometheusHandle, BuildError> {
    let handle = PrometheusBuilder::new().install_recorder()?;
    let upkeep = handle.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(METRICS_UPKEEP_INTERVAL);
        loop {
            interval.tick().await;
            upkeep.run_upkeep();
        }
    });
    Ok(handle)
}

/// `GET /metrics`: the recorder's metrics in Prometheus's text format.
pub fn metrics_router(handle: PrometheusHandle) -> Router {
    Router::new()
        .route("/metrics", get(render))
        .with_state(Arc::new(handle))
}

async fn render(State(handle): State<Arc<PrometheusHandle>>) -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        handle.render(),
    )
}

/// What a panic does to the process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanicAction {
    /// An engine call inside a request: the service catches it, the request fails with a
    /// 500, and the API logs and counts it.
    LeaveToTheService,
    /// Anywhere else (shared shell state, a background task, startup): log it and abort,
    /// since shared state may be inconsistent.
    Abort,
}

/// PRACTICES, Programmer errors panic: what a panic on this thread does.
#[must_use]
pub fn panic_action() -> PanicAction {
    if cairn_service::engine_call_active() {
        PanicAction::LeaveToTheService
    } else {
        PanicAction::Abort
    }
}

/// Installs the panic policy: a panic outside an engine call is logged and aborts the
/// process, rather than ending only the task it happened on as tokio would.
pub fn install_panic_policy() {
    std::panic::set_hook(Box::new(|info| match panic_action() {
        PanicAction::LeaveToTheService => {}
        PanicAction::Abort => {
            let location = info.location().map(ToString::to_string).unwrap_or_default();
            tracing::error!(panic = %info, location, "a panic outside an engine call: aborting");
            // Not `eprintln!`, which panics if standard error is closed.
            let _ = writeln!(std::io::stderr(), "cairn: {info}");
            std::process::abort();
        }
    }));
}
