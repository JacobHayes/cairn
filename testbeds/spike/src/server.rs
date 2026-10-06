//! The server side of the spike: an axum router over an append-only ledger.
//!
//! `POST /append` takes a request id as its body and answers with the revision at which
//! the ledger holds it; a retried id is answered with its first revision instead of being
//! appended again (the idempotency 6.1's multiplayer testbed needs, PRD H5). `GET /log`
//! answers with every id in revision order, one per line.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, oneshot};

use crate::{Bug, RequestId};

/// The ledger the server appends to, shared by every connection's handler.
#[derive(Clone)]
pub struct Ledger {
    state: Arc<Mutex<LedgerState>>,
    bug: Option<Bug>,
}

#[derive(Default)]
struct LedgerState {
    log: Vec<RequestId>,
    revisions: BTreeMap<RequestId, u64>,
    deduplicated: u64,
}

impl Ledger {
    pub fn new(bug: Option<Bug>) -> Self {
        Self {
            state: Arc::new(Mutex::new(LedgerState::default())),
            bug,
        }
    }

    /// Appends `id` unless the ledger already holds it, and returns its revision
    /// (1-based, gap-free). With the planted `no-dedupe` bug a retry is appended again.
    async fn append(&self, id: RequestId) -> u64 {
        let mut state = self.state.lock().await;
        if self.bug != Some(Bug::NoDedupe)
            && let Some(&revision) = state.revisions.get(&id)
        {
            patina_dst::reachable!("server-deduplicated-retry");
            state.deduplicated += 1;
            return revision;
        }
        state.log.push(id);
        let revision = u64::try_from(state.log.len()).unwrap_or(u64::MAX);
        state.revisions.entry(id).or_insert(revision);
        revision
    }

    /// How many appends were answered from the ledger rather than appended.
    pub async fn deduplicated(&self) -> u64 {
        self.state.lock().await.deduplicated
    }

    async fn render_log(&self) -> String {
        let state = self.state.lock().await;
        let mut text = String::new();
        for id in &state.log {
            text.push_str(&id.to_string());
            text.push('\n');
        }
        text
    }
}

async fn append(State(ledger): State<Ledger>, body: String) -> Result<String, StatusCode> {
    let id: RequestId = body.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(ledger.append(id).await.to_string())
}

async fn log(State(ledger): State<Ledger>) -> String {
    ledger.render_log().await
}

/// Serves the ledger on `listener` until `stop` fires, then shuts down gracefully:
/// in-flight connections finish before this returns.
pub async fn serve(
    listener: TcpListener,
    ledger: Ledger,
    stop: oneshot::Receiver<()>,
) -> std::io::Result<()> {
    let app = Router::new()
        .route("/append", post(append))
        .route("/log", get(log))
        .with_state(ledger);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            // A dropped sender means the run is over either way.
            let _ = stop.await;
        })
        .await
}
