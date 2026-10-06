//! Brief 6.2, the durability testbed (PRACTICES, Simulation with patina: Testbeds,
//! `durability`; PRD J2, J3, A17). The service runs on the real Turso store in one data
//! directory and one client submits a seeded plan of domain patches, recording in a durable
//! ledger what it submitted and what was acknowledged. Under the patina native shim the
//! filesystem can fail operations (EIO), shorten writes, and crash the process at a chosen
//! operation and restart it on what a power cut would leave, tearing unsynced writes.
//!
//! Every incarnation that starts on an existing ledger is a restart: it opens the store,
//! checks the invariants (`audit`) before doing anything else, resubmits the step that was
//! in flight, and finishes the plan; every run checks them again at the end. Each broken
//! invariant is a `violation` verdict and the run exits 1 (not an `always!`, whose abort
//! loses a crash-restart run's trace: DECISIONS.md, 6.2, patina); a clean run reports a
//! `pass` verdict whose detail is echoed as a `DURABILITY_RESULT` line.
//!
//! Arguments: `--dir PATH` (the data directory; default `/cairn-durability`, which exists
//! only in patina's in-memory filesystem, so a native run passes one), `--steps N` (the
//! plan's length, default 24), `--plan-seed N` (default: drawn from the run's seed),
//! `--open-attempts N` (default 1: opens tried in-process before the store counts as
//! unavailable; more only to reproduce a Turso gap), `--bug ack-before-commit` (the planted
//! client bug: a step is recorded as acknowledged before it is submitted, which only a crash
//! between the two can expose).

mod audit;
mod ledger;
mod plan;
mod restart;
mod world;

use std::path::PathBuf;
use std::process::ExitCode;

use crate::audit::Audit;
use crate::ledger::{Entry, Ledger};
use crate::plan::Plan;
use crate::world::{Stop, World};

const DEFAULT_STEPS: usize = 24;

struct Options {
    directory: PathBuf,
    steps: usize,
    plan_seed: Option<u64>,
    open_attempts: u32,
    ack_before_commit: bool,
}

fn parse_options(mut args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options {
        directory: PathBuf::from("/cairn-durability"),
        steps: DEFAULT_STEPS,
        plan_seed: None,
        open_attempts: 1,
        ack_before_commit: false,
    };
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{flag} takes a value"));
        match flag.as_str() {
            "--dir" => options.directory = PathBuf::from(value()?),
            "--steps" => {
                options.steps = value()?
                    .parse()
                    .map_err(|error| format!("--steps: {error}"))?;
            }
            "--plan-seed" => {
                let seed = value()?
                    .parse()
                    .map_err(|error| format!("--plan-seed: {error}"))?;
                options.plan_seed = Some(seed);
            }
            "--open-attempts" => {
                let attempts = value()?
                    .parse()
                    .map_err(|error| format!("--open-attempts: {error}"))?;
                options.open_attempts = attempts;
            }
            "--bug" => match value()?.as_str() {
                "ack-before-commit" => options.ack_before_commit = true,
                other => return Err(format!("unknown bug {other}")),
            },
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(options)
}

/// Reports every broken invariant as a `violation` verdict and stops the run if any broke.
fn judge(audit: &Audit, when: &str) -> Result<(), Stop> {
    for violation in &audit.violations {
        let detail = format!("{when}: {}", violation.detail);
        world::report_violation(violation.invariant.label(), &detail);
    }
    if audit.violations.is_empty() {
        Ok(())
    } else {
        Err(Stop::Violated)
    }
}

/// Reads of the store an audit may make before it gives up: injected errors fail reads
/// too, and a read is safe to repeat.
const AUDIT_ATTEMPTS: u32 = 64;

/// Audits the world's store now.
async fn check(world: &World, when: &str) -> Result<Audit, Stop> {
    let entries = world.entries();
    let mut last = String::new();
    for _ in 0..AUDIT_ATTEMPTS {
        match audit::audit(world.store(), world.service(), &world.plan, &entries).await {
            Ok(audit) => {
                judge(&audit, when)?;
                return Ok(audit);
            }
            Err(error) => last = error.to_string(),
        }
    }
    Err(Stop::Liveness(format!(
        "the audit could not read the store: {last}"
    )))
}

/// The ledger's plan, or a new one recorded as its first entry.
fn plan_of(ledger: &mut Ledger, options: &Options) -> Result<Plan, Stop> {
    if let Some(Entry::Plan { seed, length }) = ledger.entries().first() {
        return Ok(Plan::generate(*seed, *length));
    }
    let seed = options.plan_seed.unwrap_or_else(patina_dst::rng);
    let recorded = ledger.append(Entry::Plan {
        seed,
        length: options.steps,
    });
    recorded.map_err(|error| Stop::Abort {
        label: "durability-ledger-writable",
        detail: error.to_string(),
    })?;
    Ok(Plan::generate(seed, options.steps))
}

async fn run(options: &Options) -> Result<String, Stop> {
    let mut ledger = Ledger::open(&options.directory).map_err(|error| Stop::Abort {
        label: "durability-ledger-writable",
        detail: error.to_string(),
    })?;
    let restarted = !ledger.entries().is_empty();
    let plan = plan_of(&mut ledger, options)?;
    let before = restart::Before::read(&options.directory, &ledger)?;
    let mut world = World::open(
        options.directory.clone(),
        plan,
        ledger,
        options.open_attempts,
    )
    .await?;
    world.ack_before_commit = options.ack_before_commit;
    let mut resubmit = None;
    if restarted {
        world.record(Entry::Restart)?;
        let audit = check(&world, "after-restart").await?;
        resubmit = before.observe(&audit);
    }
    patina_dst::lifecycle::setup_complete();
    let acknowledged = restart::acknowledged(&world);
    for index in 0..world.plan.steps.len() {
        if acknowledged.contains(&index) {
            continue;
        }
        if patina_dst::buggify!("durability-reopen-between-steps") {
            world.reopen().await?;
            check(&world, "after-reopen").await?;
        }
        let written = world.submit(index).await?;
        if let Some((in_flight, landed)) = resubmit.take() {
            restart::judge_resubmission(in_flight, index, landed, &written)?;
        }
    }
    let audit = check(&world, "at-end").await?;
    Ok(format!(
        "outcome=complete plan_seed={} steps={} committed={} events={} restarted={restarted} reopened={} failures={} state={:016x}",
        world.plan.seed,
        world.plan.steps.len(),
        audit.committed.len(),
        audit.events,
        world.reopened,
        world.failures,
        audit.digest,
    ))
}

fn main() -> ExitCode {
    let options = match parse_options(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("durability: {message}");
            return ExitCode::from(64);
        }
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return abort("durability-runtime-starts", &error.to_string()),
    };
    match runtime.block_on(run(&options)) {
        Ok(detail) => {
            patina_dst::verdict(patina_dst::VerdictKind::Pass, "durability-outcome", &detail);
            eprintln!("DURABILITY_RESULT {detail}");
            ExitCode::SUCCESS
        }
        Err(Stop::Abort { label, detail }) => abort(label, &detail),
        Err(Stop::Violated) => ExitCode::FAILURE,
        Err(Stop::Unavailable(detail)) => unavailable(&detail),
        Err(Stop::Liveness(detail)) => {
            eprintln!("DURABILITY_FAILURE {detail}");
            ExitCode::from(4)
        }
    }
}

/// A store that did not open: under injected I/O errors and short reads, an honest answer
/// with nothing committed to judge, so the run passes as `unavailable`. The crash sweep
/// injects neither and requires every run to complete (`sim.sh`), which is where a store
/// that does not open after a crash fails.
fn unavailable(detail: &str) -> ExitCode {
    let detail = format!("outcome=unavailable {detail}");
    patina_dst::verdict(patina_dst::VerdictKind::Pass, "durability-outcome", &detail);
    eprintln!("DURABILITY_RESULT {detail}");
    ExitCode::SUCCESS
}

/// Announces a deliberate stop through the verdict channel, so the exit is attributed to
/// the testbed rather than read as a patina refusal. Never exit code 2: patina reserves it.
fn abort(label: &str, detail: &str) -> ExitCode {
    patina_dst::verdict(patina_dst::VerdictKind::AbortIntent, label, detail);
    eprintln!("DURABILITY_ABORT {label} {detail}");
    ExitCode::from(3)
}
