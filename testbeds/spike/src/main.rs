//! Brief 1.3, the simulation spike: can tokio, axum, and a hyper client run under the
//! patina native shim (PRACTICES, Simulation with patina)?
//!
//! One process runs an axum server and a few hyper clients on tokio (one current-thread
//! runtime, one multi-thread runtime, or a current-thread runtime per client thread, chosen
//! by `--flavor`), over the loopback network patina simulates. Each client appends its own
//! request ids to the server's ledger, one HTTP request at a time, retrying an attempt that
//! fails or times out; the server deduplicates a retried id (PRD H5). At the end one more
//! request reads the whole ledger and the run checks the invariants 6.1's multiplayer
//! testbed names: every acknowledged request is in the ledger at its acknowledged revision,
//! and no request is applied twice.
//!
//! Outcomes go through patina's verdict channel: a `pass` under `spike-outcome` whose
//! detail carries the counts, or a `violation` under the invariant's label. Exit codes: 0
//! pass, 1 violation, 3 abort (the runtime or the listener could not start), 4 a client gave
//! up or the server would not stop (liveness, no verdict), 64 bad arguments.
//!
//! `--tick-ms N` adds a task that wakes the runtime every N milliseconds, the workaround for
//! the delayed-delivery gap the README reports.
//!
//! `--bug no-dedupe` plants a server that appends a retried id again: invisible on a
//! fault-free run, caught by `applied-once` once a fault forces a retry.

mod client;
mod outcome;
mod server;

use std::fmt;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::process::ExitCode;
use std::str::FromStr;
use std::time::Duration;

use tokio::sync::oneshot;
use tokio::task::JoinSet;

use outcome::Outcome;

/// The server's address: an IP literal, since the shim resolves no names without a host
/// table.
const SERVER: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 7878));
/// How long a graceful server shutdown may take once every client is done.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);
/// Worker threads for the multi-thread flavor: more than one, so tasks really migrate.
const WORKER_THREADS: usize = 2;

/// Which tokio runtimes drive the run.
#[derive(Clone, Copy, Debug)]
enum Flavor {
    /// One current-thread runtime for the server and every client.
    CurrentThread,
    /// One multi-thread runtime for the server and every client.
    MultiThread,
    /// The server on a current-thread runtime; each client on its own `std::thread` with
    /// its own current-thread runtime (6.1's shape: client threads beside the server).
    ThreadPerClient,
}

/// A planted bug, chosen with `--bug`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bug {
    /// The server appends a retried id again instead of answering with its revision.
    NoDedupe,
}

/// One request: a client's sequence number. Its text form `client-sequence` is the body
/// of an append and a line of the log.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RequestId {
    pub client: u32,
    pub sequence: u32,
}

impl fmt::Display for RequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}-{}", self.client, self.sequence)
    }
}

impl FromStr for RequestId {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (client, sequence) = text
            .trim()
            .split_once('-')
            .ok_or_else(|| format!("request id {text:?} is not client-sequence"))?;
        let number = |part: &str| {
            part.parse()
                .map_err(|_| format!("request id {text:?} is not client-sequence"))
        };
        Ok(Self {
            client: number(client)?,
            sequence: number(sequence)?,
        })
    }
}

#[derive(Debug)]
struct Options {
    flavor: Flavor,
    clients: u32,
    requests: u32,
    bug: Option<Bug>,
    /// The period of the ticker task, if any (`--tick-ms`, 0 for none).
    tick: Option<Duration>,
}

fn parse_options(mut args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options {
        flavor: Flavor::CurrentThread,
        clients: 3,
        requests: 8,
        bug: None,
        tick: None,
    };
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match (flag.as_str(), value.as_str()) {
            ("--flavor", "current-thread") => options.flavor = Flavor::CurrentThread,
            ("--flavor", "multi-thread") => options.flavor = Flavor::MultiThread,
            ("--flavor", "thread-per-client") => options.flavor = Flavor::ThreadPerClient,
            ("--clients", count) => options.clients = parse_count(&flag, count)?,
            ("--requests", count) => options.requests = parse_count(&flag, count)?,
            ("--bug", "no-dedupe") => options.bug = Some(Bug::NoDedupe),
            ("--tick-ms", "0") => options.tick = None,
            ("--tick-ms", period) => {
                options.tick = Some(Duration::from_millis(parse_count(&flag, period)?.into()));
            }
            _ => return Err(format!("unknown argument {flag} {value}")),
        }
    }
    Ok(options)
}

fn parse_count(flag: &str, value: &str) -> Result<u32, String> {
    match value.parse() {
        Ok(count) if count > 0 => Ok(count),
        _ => Err(format!("{flag} takes a positive count, not {value:?}")),
    }
}

fn build_runtime(flavor: Flavor) -> std::io::Result<tokio::runtime::Runtime> {
    let mut builder = match flavor {
        Flavor::CurrentThread | Flavor::ThreadPerClient => {
            tokio::runtime::Builder::new_current_thread()
        }
        Flavor::MultiThread => {
            let mut builder = tokio::runtime::Builder::new_multi_thread();
            builder.worker_threads(WORKER_THREADS);
            builder
        }
    };
    builder.enable_all().build()
}

/// Why a run ended without an outcome to judge.
enum Stop {
    /// The run could not start: announced as an abort intent.
    Abort { label: &'static str, detail: String },
    /// A liveness miss: a client gave up, or the server would not stop.
    Liveness(String),
}

impl From<client::GaveUp> for Stop {
    fn from(gave_up: client::GaveUp) -> Self {
        Self::Liveness(format!(
            "gave up on {}: {}",
            gave_up.what, gave_up.last_error
        ))
    }
}

/// Wakes the runtime every `period` and does nothing else. Under the shim a TCP segment
/// delayed by a network fault is delivered only when some timer wakes the runtime (the
/// patina gap in the README); the ticker bounds that lag to `period`.
async fn tick(period: Duration) {
    let mut interval = tokio::time::interval(period);
    loop {
        interval.tick().await;
    }
}

/// Runs every client to completion, as tasks on this runtime or, for
/// [`Flavor::ThreadPerClient`], on threads of their own.
async fn run_clients(options: &Options) -> Result<Vec<client::Report>, Stop> {
    let mut reports = Vec::new();
    if let Flavor::ThreadPerClient = options.flavor {
        let threads: Vec<_> = (0..options.clients)
            .map(|client| spawn_client_thread(client, options.requests, options.tick))
            .collect();
        for thread in threads {
            let result = thread
                .await
                .map_err(|_| Stop::Liveness("client thread ended without a report".to_owned()))?;
            reports.push(result?);
        }
        return Ok(reports);
    }
    let mut clients = JoinSet::new();
    for client in 0..options.clients {
        clients.spawn(client::run_client(SERVER, client, options.requests));
    }
    while let Some(joined) = clients.join_next().await {
        let report =
            joined.map_err(|error| Stop::Liveness(format!("client task failed: {error}")))??;
        reports.push(report);
    }
    Ok(reports)
}

/// Runs one client on a new `std::thread` with its own current-thread runtime (and
/// ticker), and hands its report back over a channel.
fn spawn_client_thread(
    client: u32,
    requests: u32,
    period: Option<Duration>,
) -> oneshot::Receiver<Result<client::Report, Stop>> {
    let (report, reported) = oneshot::channel();
    std::thread::spawn(move || {
        let result = match build_runtime(Flavor::CurrentThread) {
            Ok(runtime) => runtime.block_on(async move {
                let ticker = period.map(|period| tokio::spawn(tick(period)));
                let result = client::run_client(SERVER, client, requests).await;
                if let Some(ticker) = ticker {
                    ticker.abort();
                }
                Ok(result?)
            }),
            Err(error) => Err(Stop::Abort {
                label: "runtime",
                detail: error.to_string(),
            }),
        };
        // The receiver is gone only if the run already stopped.
        let _ = report.send(result);
    });
    reported
}

/// Starts the server, runs every client to completion, reads the log, and stops the server.
async fn run(options: &Options) -> Result<Outcome, Stop> {
    let listener = tokio::net::TcpListener::bind(SERVER)
        .await
        .map_err(|error| Stop::Abort {
            label: "bind",
            detail: error.to_string(),
        })?;
    let ledger = server::Ledger::new(options.bug);
    let (stop_server, stopped) = oneshot::channel();
    let server_task = tokio::spawn(server::serve(listener, ledger.clone(), stopped));
    let ticker = options.tick.map(|period| tokio::spawn(tick(period)));
    patina_dst::lifecycle::setup_complete();
    let started = tokio::time::Instant::now();

    let reports = run_clients(options).await?;
    let (log, log_attempts) = client::fetch_log(SERVER).await?;
    let elapsed = started.elapsed();

    if let Some(ticker) = ticker {
        ticker.abort();
    }
    let _ = stop_server.send(());
    match tokio::time::timeout(SHUTDOWN_TIMEOUT, server_task).await {
        Ok(Ok(Ok(()))) => {}
        Ok(Ok(Err(error))) => return Err(Stop::Liveness(format!("server failed: {error}"))),
        Ok(Err(error)) => return Err(Stop::Liveness(format!("server task failed: {error}"))),
        Err(_) => return Err(Stop::Liveness("server did not stop".to_owned())),
    }
    Ok(Outcome::new(
        &reports,
        &log,
        u64::from(log_attempts),
        ledger.deduplicated().await,
        elapsed,
    ))
}

fn main() -> ExitCode {
    let options = match parse_options(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("spike: {message}");
            return ExitCode::from(64);
        }
    };
    let runtime = match build_runtime(options.flavor) {
        Ok(runtime) => runtime,
        Err(error) => return abort("runtime", &error.to_string()),
    };
    let flavor = format!("{:?}", options.flavor);
    match runtime.block_on(run(&options)) {
        Ok(outcome) => outcome.judge(&flavor, options.clients * options.requests),
        Err(Stop::Abort { label, detail }) => abort(label, &detail),
        Err(Stop::Liveness(detail)) => {
            eprintln!("SPIKE_FAILURE flavor={flavor} {detail}");
            ExitCode::from(4)
        }
    }
}

/// Announces a deliberate stop through the verdict channel, so the exit is attributed to
/// the spike rather than read as a patina refusal.
fn abort(label: &str, detail: &str) -> ExitCode {
    patina_dst::verdict(patina_dst::VerdictKind::AbortIntent, label, detail);
    eprintln!("SPIKE_ABORT {label} {detail}");
    ExitCode::from(3)
}
