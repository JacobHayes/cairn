//! The client side of the spike: hyper's HTTP/1 connection over a tokio `TcpStream`, one
//! fresh connection per attempt, with a per-attempt timeout and retries.
//!
//! The retry loop is the behaviour faults should reach: a reset, a refused connect, or a
//! delay past the attempt timeout fails the attempt, and the same request id is sent again,
//! which the server must deduplicate.

use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::client::conn::http1;
use hyper::{Method, Request, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;

use crate::RequestId;

/// One attempt's budget: connect, send, and read the whole response. Several times the
/// round trip under patina's millisecond-scale network delays, so only a delay that stacks
/// up (or the delayed-delivery gap, without a ticker) times an attempt out.
const ATTEMPT_TIMEOUT: Duration = Duration::from_millis(40);
/// The pause before a retry.
const RETRY_BACKOFF: Duration = Duration::from_millis(5);
/// Attempts per request before the client gives up.
const ATTEMPTS_MAX: u32 = 64;

/// Why one attempt failed.
#[derive(Debug)]
pub enum AttemptError {
    Io(std::io::Error),
    Http(hyper::Error),
    Status(StatusCode),
    TimedOut,
    /// The connection ended (closed or reset) before the response arrived.
    Closed,
    /// The `client-loses-response` fault site fired: the server answered, the client
    /// dropped the answer.
    ResponseLost,
    Malformed(String),
}

impl fmt::Display for AttemptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "io: {error}"),
            Self::Http(error) => write!(formatter, "http: {error}"),
            Self::Status(status) => write!(formatter, "status {status}"),
            Self::TimedOut => write!(formatter, "timed out"),
            Self::Closed => write!(formatter, "connection ended before the response"),
            Self::ResponseLost => write!(formatter, "response lost (fault site)"),
            Self::Malformed(body) => write!(formatter, "malformed body {body:?}"),
        }
    }
}

/// A request that failed every attempt.
#[derive(Debug)]
pub struct GaveUp {
    pub what: String,
    pub last_error: AttemptError,
}

/// What one client saw: each request's acknowledged revision, and how many attempts it took.
#[derive(Debug, Default)]
pub struct Report {
    pub acknowledged: Vec<(RequestId, u64)>,
    pub attempts: u64,
}

/// Appends `requests` ids for `client`, one after another, retrying each until acknowledged.
pub async fn run_client(server: SocketAddr, client: u32, requests: u32) -> Result<Report, GaveUp> {
    let mut report = Report::default();
    for sequence in 0..requests {
        let id = RequestId { client, sequence };
        let (body, attempts) = with_retries(&id.to_string(), |attempt| {
            exchange(server, Method::POST, "/append", id.to_string(), attempt)
        })
        .await?;
        report.attempts += u64::from(attempts);
        let revision = body.trim().parse().map_err(|_| GaveUp {
            what: format!("append {id}"),
            last_error: AttemptError::Malformed(body.clone()),
        })?;
        report.acknowledged.push((id, revision));
    }
    Ok(report)
}

/// Reads the server's whole log, retrying like an append.
pub async fn fetch_log(server: SocketAddr) -> Result<(String, u32), GaveUp> {
    with_retries("log", |attempt| {
        exchange(server, Method::GET, "/log", String::new(), attempt)
    })
    .await
}

/// Runs `attempt` (given its 1-based number) until it succeeds within [`ATTEMPT_TIMEOUT`],
/// at most [`ATTEMPTS_MAX`] times, and returns its body with the number of attempts made.
async fn with_retries<F, Fut>(what: &str, mut attempt: F) -> Result<(String, u32), GaveUp>
where
    F: FnMut(u32) -> Fut,
    Fut: Future<Output = Result<String, AttemptError>>,
{
    let mut last_error = AttemptError::TimedOut;
    for made in 1..=ATTEMPTS_MAX {
        let outcome = tokio::time::timeout(ATTEMPT_TIMEOUT, attempt(made))
            .await
            .unwrap_or(Err(AttemptError::TimedOut));
        match outcome {
            Ok(body) => return Ok((body, made)),
            Err(error) => {
                eprintln!("SPIKE_RETRY {what} attempt={made} {error}");
                last_error = error;
            }
        }
        patina_dst::reachable!("client-retried");
        tokio::time::sleep(RETRY_BACKOFF).await;
    }
    Err(GaveUp {
        what: what.to_owned(),
        last_error,
    })
}

/// One HTTP/1 exchange on a fresh connection: the response body on a 200. The
/// `client-loses-response` fault site may drop a first attempt's answer, so a seed that
/// fires it everywhere still finishes on the retry.
async fn exchange(
    server: SocketAddr,
    method: Method,
    path: &str,
    body: String,
    attempt: u32,
) -> Result<String, AttemptError> {
    let stream = TcpStream::connect(server).await.map_err(AttemptError::Io)?;
    let (mut sender, connection) = http1::handshake(TokioIo::new(stream))
        .await
        .map_err(AttemptError::Http)?;
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(hyper::header::HOST, server.to_string())
        .body(Full::new(Bytes::from(body)))
        .map_err(|error| AttemptError::Malformed(error.to_string()))?;
    let response = async {
        let response = sender
            .send_request(request)
            .await
            .map_err(AttemptError::Http)?;
        if attempt == 1 && patina_dst::buggify!("client-loses-response") {
            return Err(AttemptError::ResponseLost);
        }
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .map_err(AttemptError::Http)?
            .to_bytes();
        if status != StatusCode::OK {
            return Err(AttemptError::Status(status));
        }
        String::from_utf8(bytes.to_vec())
            .map_err(|error| AttemptError::Malformed(error.to_string()))
    };
    // The connection future drives the socket; the exchange ends first on success, and the
    // connection (and its socket) is dropped with it.
    tokio::select! {
        biased;
        result = response => result,
        closed = connection => match closed {
            Ok(()) => Err(AttemptError::Closed),
            Err(error) => Err(AttemptError::Http(error)),
        },
    }
}
