//! The derive benchmark at the limits (ARCHITECTURE, Date network: reported, not gated):
//! elapsed time of the plan check and of a whole derive over `generated::date_limits`, and
//! the process's peak resident memory, natively. The browser's wasm half is the wasm host's
//! (brief 4.5). Run it optimized:
//!
//! ```text
//! cargo run --release -p cairn-engine --example date_benchmark
//! ```
//!
//! Prints a Markdown table. Peak memory is read from `/proc/self/status` (Linux); elsewhere
//! that row says so.

use std::time::{Duration, Instant};

use cairn_engine::testing::derive_inputs;
use cairn_engine::testing::generated::date_limits;
use cairn_engine::{check_plan, derive};
use cairn_schema::Deployment;

const RUNS: u32 = 5;

/// A field of `/proc/self/status` in KiB, such as `VmRSS` or `VmHWM`.
fn status_kib(field: &str) -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with(field))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

/// The median of `RUNS` timings of `work`.
fn median(mut work: impl FnMut()) -> Duration {
    let mut times: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            work();
            started.elapsed()
        })
        .collect();
    times.sort();
    times.get(times.len() / 2).copied().unwrap_or_default()
}

fn main() {
    let graph = date_limits();
    let deployment = Deployment::default();
    let inputs = derive_inputs(deployment.clone());
    let resident_before = status_kib("VmRSS");
    let derived = derive(&graph, None, &inputs);
    let (instants, constraints) = derived.dates().network_size();
    let plan = median(|| assert!(check_plan(&graph, &deployment).is_consistent()));
    let whole = median(|| {
        let _ = derive(&graph, None, &inputs);
    });
    let peak = status_kib("VmHWM");
    let memory = |kib: Option<u64>| {
        kib.map_or_else(
            || "not measured (no /proc/self/status)".to_owned(),
            |kib| format!("{} MiB", kib / 1024),
        )
    };
    println!("| Measure | Value |");
    println!("|---|---|");
    println!("| nodes | {} |", graph.document().nodes.len());
    println!("| instants, constraints | {instants}, {constraints} |");
    println!(
        "| plan check, median of {RUNS} | {:.1} ms |",
        plan.as_secs_f64() * 1000.0
    );
    println!(
        "| whole derive (passes 1 to 4), median of {RUNS} | {:.1} ms |",
        whole.as_secs_f64() * 1000.0
    );
    println!(
        "| resident memory with the graph built | {} |",
        memory(resident_before)
    );
    println!("| peak resident memory | {} |", memory(peak));
}
