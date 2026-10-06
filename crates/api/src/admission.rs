//! The limits requests are held to (PRACTICES, Explicit limits): the request body size and
//! the requests in flight, for every request, and the request duration, for the API's own
//! endpoints only: auth's routes and layer make outbound identity calls under their own,
//! longer limit (`IDENTITY_CALL_DURATION_MAX`). The SSE subscriber limit is the notifier's,
//! and the stream enforces its own write stall.

use std::sync::Arc;

use axum::Router;
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use tokio::sync::Semaphore;

use crate::error::ApiError;
use crate::limits::{REQUEST_BYTES_MAX, REQUEST_DURATION_MAX, REQUEST_IN_FLIGHT_COUNT_MAX};
use crate::wire::ProblemCode;

/// `router` with the body and in-flight limits applied to every request.
pub fn limited(router: Router) -> Router {
    let in_flight = Arc::new(Semaphore::new(REQUEST_IN_FLIGHT_COUNT_MAX as usize));
    let body_bytes_max = usize::try_from(REQUEST_BYTES_MAX).unwrap_or(usize::MAX);
    router
        .layer(DefaultBodyLimit::max(body_bytes_max))
        .layer(middleware::from_fn_with_state(in_flight, admit))
}

/// `router` with the request duration limit: for the API's endpoints, not auth's.
pub fn timed(router: Router) -> Router {
    router.layer(middleware::from_fn(duration))
}

/// Admits a request while fewer than `REQUEST_IN_FLIGHT_COUNT_MAX` are in flight, holding
/// its slot until its response is ready; past that, 503 with `Retry-After`. An SSE stream
/// holds a slot only until its response starts: its subscriber limit is the notifier's.
async fn admit(State(in_flight): State<Arc<Semaphore>>, request: Request, next: Next) -> Response {
    let Ok(_slot) = in_flight.try_acquire() else {
        let message = format!(
            "{REQUEST_IN_FLIGHT_COUNT_MAX} requests are in flight (request_in_flight_count_max)"
        );
        return ApiError::problem(ProblemCode::Overloaded, message).into_response();
    };
    next.run(request).await
}

/// Answers a request that runs past `REQUEST_DURATION_MAX` as timed out. Its handler is
/// dropped where it stood; a patch already handed to the service runs to the end on its
/// own task, so a commit is never cut off before its announcement.
async fn duration(request: Request, next: Next) -> Response {
    if let Ok(response) = tokio::time::timeout(REQUEST_DURATION_MAX, next.run(request)).await {
        return response;
    }
    let message = format!(
        "the request ran past {} s (request_duration_max)",
        REQUEST_DURATION_MAX.as_secs()
    );
    ApiError::problem(ProblemCode::TimedOut, message).into_response()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use tower::ServiceExt;

    use super::*;

    fn status(router: &Router, path: &str) -> impl Future<Output = StatusCode> + use<> {
        let request = Request::get(path).body(Body::empty()).unwrap();
        let answer = router.clone().oneshot(request);
        async move { answer.await.unwrap().status() }
    }

    #[tokio::test(start_paused = true)]
    async fn only_a_timed_request_past_the_duration_limit_is_answered_503() {
        let routes = || {
            Router::new()
                .route("/slow", get(|| tokio::time::sleep(Duration::from_secs(6))))
                .route("/quick", get(|| tokio::time::sleep(Duration::from_secs(4))))
        };
        let timed_routes = limited(timed(routes()));
        assert_eq!(status(&timed_routes, "/quick").await, StatusCode::OK);
        assert_eq!(
            status(&timed_routes, "/slow").await,
            StatusCode::SERVICE_UNAVAILABLE
        );
        let untimed = limited(routes());
        assert_eq!(
            status(&untimed, "/slow").await,
            StatusCode::OK,
            "auth's routes"
        );
    }

    #[tokio::test]
    async fn past_the_in_flight_limit_a_request_is_answered_503_until_one_finishes() {
        let gate = Arc::new(tokio::sync::Notify::new());
        let entered = Arc::new(AtomicU32::new(0));
        let (held, count) = (Arc::clone(&gate), Arc::clone(&entered));
        let router = limited(
            Router::new()
                .route(
                    "/held",
                    get(move || {
                        let notified = Arc::clone(&held).notified_owned();
                        count.fetch_add(1, Ordering::SeqCst);
                        notified
                    }),
                )
                .route("/quick", get(|| async {})),
        );
        let mut waiting = Vec::new();
        for _ in 0..REQUEST_IN_FLIGHT_COUNT_MAX {
            waiting.push(tokio::spawn(status(&router, "/held")));
        }
        while entered.load(Ordering::SeqCst) < REQUEST_IN_FLIGHT_COUNT_MAX {
            tokio::task::yield_now().await;
        }
        assert_eq!(
            status(&router, "/quick").await,
            StatusCode::SERVICE_UNAVAILABLE
        );
        gate.notify_waiters();
        for answer in waiting {
            assert_eq!(answer.await.unwrap(), StatusCode::OK);
        }
        assert_eq!(status(&router, "/quick").await, StatusCode::OK);
    }
}
