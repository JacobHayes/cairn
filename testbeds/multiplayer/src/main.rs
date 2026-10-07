//! Brief 6.1, the multiplayer testbed (PRACTICES, Simulation with patina: Testbeds).
//!
//! One process runs Cairn's real HTTP server (4.2's router over a service on the Turso
//! store) and N virtual clients, all as tasks on one current-thread tokio runtime (the
//! configuration the spike, 1.3, found the patina shim carries). Each client patches one
//! shared journey through the real Rust client: H5's safe retry resubmits a stale patch
//! when what intervened cannot overlap it and surfaces it when it can, and a transport
//! retry resends a patch whose answer was lost, which the server answers from its receipt.
//! Each client also keeps a view current through its SSE subscription and H6's tracking.
//! One more client is an agent calling the service in process, with the same retry logic
//! (4.2's `client::retry`), as an agent host does.
//!
//! When every client is done the run checks each invariant (`outcome.rs`): every
//! acknowledged patch was visible in its client's next snapshot and sits in the log at its
//! revision; no patch was applied twice; nothing unacknowledged or surfaced as a conflict
//! was applied; nothing but conflicts was refused; every domain's revisions are gap-free; a
//! patch resubmitted on its own landed only over commits that do not touch what it
//! touches; replaying the log equals the state; and every view reached the store's
//! revisions within the catch-up deadline after writes stopped. `sometimes!` oracles show
//! the interesting paths ran (`client.rs`, `view.rs`), and fault sites in the store make
//! commits overlap (`faults.rs`).
//!
//! Exit codes: 0 pass, 1 violation (a `violation` verdict per broken invariant), 3 abort (the runtime, store, or listener could not start), 4 a
//! liveness miss (a client gave up, a view never caught up, or the server would not stop),
//! 64 bad arguments. Never 2, which patina reserves.

mod client;
mod faults;
mod outcome;
mod patches;
mod view;
mod world;

use std::process::ExitCode;
use std::time::Duration;

use tokio::task::JoinSet;

use crate::client::{Client, GaveUp};
use crate::outcome::{Outcome, Truth};

/// What a run is told on its command line.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Options {
    /// Virtual clients.
    clients: u32,
    /// Patches each client drafts.
    actions: u32,
    /// The period of the ticker task, if any (`--tick-ms`, 0 for none).
    tick: Option<Duration>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            clients: 4,
            actions: 8,
            tick: Some(Duration::from_millis(1)),
        }
    }
}

fn parse_options(mut args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options::default();
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--clients" => options.clients = parse_count(&flag, &value)?,
            "--actions" => options.actions = parse_count(&flag, &value)?,
            "--tick-ms" if value == "0" => options.tick = None,
            "--tick-ms" => {
                options.tick = Some(Duration::from_millis(parse_count(&flag, &value)?.into()));
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

/// Why a run ended without an outcome to judge.
#[derive(Debug)]
pub enum Stop {
    /// The run could not start: announced as an abort intent.
    Abort { label: &'static str, detail: String },
    /// A liveness miss.
    Liveness(String),
}

impl From<GaveUp> for Stop {
    fn from(gave_up: GaveUp) -> Self {
        Self::Liveness(gave_up.to_string())
    }
}

/// Wakes the runtime every `period` and does nothing else: under the shim a TCP segment
/// delayed by a network fault is delivered only when some timer wakes the runtime
/// (decisions/2026-10-06-patina-a-delayed-tcp-segment-does-not-wake-epoll-wait.md).
async fn tick(period: Duration) {
    let mut interval = tokio::time::interval(period);
    loop {
        interval.tick().await;
    }
}

/// Starts the server, sets up the shared journey, runs every client and view, waits for
/// the views to catch up, reads what the store holds, and stops the server.
async fn run(options: &Options) -> Result<Outcome, Stop> {
    let ticker = options.tick.map(|period| tokio::spawn(tick(period)));
    let server = world::Server::start().await?;
    let setup = world::set_up(&Client::new(world::SERVER, u32::MAX)).await?;
    server.arm();
    patina_dst::lifecycle::setup_complete();
    let started = tokio::time::Instant::now();

    let mut views = Vec::new();
    for client in 0..options.clients {
        views.push(view::View::open(world::SERVER, client, &setup));
    }
    let mut clients = JoinSet::new();
    for client in 0..options.clients {
        let driver = Client::new(world::SERVER, client);
        let setup = setup.clone();
        let actions = options.actions;
        clients.spawn(async move { driver.run(&setup, actions).await });
    }
    // One agent beside them, calling the service in process as an agent host does.
    let agent = Client::in_process(server.service().clone(), options.clients);
    let (agent_setup, actions) = (setup.clone(), options.actions);
    clients.spawn(async move { agent.run(&agent_setup, actions).await });
    let mut reports = Vec::new();
    while let Some(joined) = clients.join_next().await {
        let report =
            joined.map_err(|error| Stop::Liveness(format!("client task failed: {error}")))??;
        reports.push(report);
    }
    let writes_stopped = started.elapsed();

    let truth = Truth::read(server.service(), &setup).await?;
    let caught_up = view::catch_up(&mut views, &truth.revisions()).await;
    for view in views {
        view.close();
    }
    let outcome = Outcome::new(&setup, reports, truth, caught_up, writes_stopped);
    server.stop().await?;
    if let Some(ticker) = ticker {
        ticker.abort();
    }
    Ok(outcome)
}

fn main() -> ExitCode {
    let options = match parse_options(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("multiplayer: {message}");
            return ExitCode::from(64);
        }
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return abort("runtime", &error.to_string()),
    };
    match runtime.block_on(run(&options)) {
        Ok(outcome) => outcome.judge(),
        Err(Stop::Abort { label, detail }) => abort(label, &detail),
        Err(Stop::Liveness(detail)) => {
            eprintln!("MULTIPLAYER_FAILURE {detail}");
            ExitCode::from(4)
        }
    }
}

/// Announces a deliberate stop through the verdict channel, so the exit is attributed to
/// the testbed rather than read as a patina refusal.
fn abort(label: &str, detail: &str) -> ExitCode {
    patina_dst::verdict(patina_dst::VerdictKind::AbortIntent, label, detail);
    eprintln!("MULTIPLAYER_ABORT {label} {detail}");
    ExitCode::from(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, String> {
        parse_options(args.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn options_parse_and_refuse_what_they_should() {
        assert_eq!(parse(&[]), Ok(Options::default()));
        let parsed = parse(&["--clients", "6", "--actions", "3", "--tick-ms", "0"]).unwrap();
        assert_eq!(
            parsed,
            Options {
                clients: 6,
                actions: 3,
                tick: None
            }
        );
        for bad in [
            &["--clients"][..],
            &["--clients", "0"],
            &["--actions", "x"],
            &["--bug", "y"],
        ] {
            assert!(parse(bad).is_err(), "{bad:?} parsed");
        }
    }
}
