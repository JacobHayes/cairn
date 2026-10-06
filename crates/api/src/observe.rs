//! Observability on every handler (ARCHITECTURE, Observability; PRD Non-functional:
//! Observable): each request gets an id, echoed in `x-request-id` and in every problem
//! body, a tracing span that its handler's logs nest in, and a log line, a count, and a
//! latency when it is answered, by endpoint. Patches add their accept and reject counts;
//! a caught engine panic adds to the panic count. The metrics go through the `metrics`
//! facade, to whatever exporter the binary installs.

use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use tracing::Instrument;

/// Requests answered, by endpoint, method, and status.
pub const REQUESTS: &str = "cairn_api_requests_total";
/// Request latency in seconds, by endpoint and method.
pub const REQUEST_DURATION: &str = "cairn_api_request_duration_seconds";
/// Domain patches answered, by outcome: `applied`, `already_applied`, `invalid`, `stale`,
/// `patch_id_reused`.
pub const PATCHES: &str = "cairn_api_patches_total";
/// Engine panics caught in a request (PRACTICES, Programmer errors panic).
pub const ENGINE_PANICS: &str = "cairn_engine_panics_total";

/// The response header carrying the request id.
pub const REQUEST_ID_HEADER: &str = "x-request-id";

tokio::task_local! {
    static REQUEST_ID: String;
}

/// The id of the request being handled on this task, if any.
#[must_use]
pub fn current_request_id() -> Option<String> {
    REQUEST_ID.try_with(Clone::clone).ok()
}

/// A fresh request id: 16 random hex digits, or, if the random source fails, a count.
fn mint_request_id() -> String {
    static FALLBACK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut bytes = [0_u8; 8];
    let random = match getrandom::fill(&mut bytes) {
        Ok(()) => u64::from_be_bytes(bytes),
        Err(_) => FALLBACK.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    };
    format!("rq_{random:016x}")
}

/// The middleware: an id and a span for the request, then its log line and metrics.
pub async fn request(request: Request, next: Next) -> Response {
    let id = mint_request_id();
    let method = request.method().clone();
    let endpoint = request
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| "unmatched".to_owned(), |path| path.as_str().to_owned());
    let span = tracing::info_span!(
        "request",
        request_id = %id,
        method = %method,
        endpoint = %endpoint,
    );
    let started = Instant::now();
    let mut response = REQUEST_ID
        .scope(id.clone(), next.run(request))
        .instrument(span.clone())
        .await;
    let elapsed = started.elapsed();
    let status = response.status();
    span.in_scope(|| {
        tracing::info!(
            status = status.as_u16(),
            duration_ms = elapsed.as_millis(),
            "answered"
        );
    });
    let labels = [
        ("endpoint", endpoint),
        ("method", method.as_str().to_owned()),
    ];
    let mut counted = labels.to_vec();
    counted.push(("status", status.as_u16().to_string()));
    metrics::counter!(REQUESTS, &counted).increment(1);
    metrics::histogram!(REQUEST_DURATION, &labels).record(elapsed.as_secs_f64());
    if let Ok(value) = HeaderValue::from_str(&id) {
        response.headers_mut().insert(REQUEST_ID_HEADER, value);
    }
    response
}

/// Counts a domain patch's outcome.
pub fn patch_outcome(outcome: &'static str) {
    metrics::counter!(PATCHES, "outcome" => outcome).increment(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_ids_are_distinct_and_only_inside_a_request() {
        assert_ne!(mint_request_id(), mint_request_id());
        assert_eq!(current_request_id(), None);
        let inside = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(REQUEST_ID.scope("rq_1".to_owned(), async { current_request_id() }));
        assert_eq!(inside.as_deref(), Some("rq_1"));
    }
}
