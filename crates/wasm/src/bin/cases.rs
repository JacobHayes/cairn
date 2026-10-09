//! Writes the agreement cases for the browser tests (brief 4.5): every group the server walk
//! gives and the document at the limits, one JSON file each, with an index, into the
//! directory named first. With `--bench`, also times the derive at the limits natively
//! through this crate (the browser's half of ARCHITECTURE, Date network's benchmark is the
//! browser test's) and writes `bench-native.json` beside them. Run it optimized for the
//! benchmark:
//!
//! ```text
//! cargo run --release -p cairn-wasm --features server --bin cairn-wasm-cases -- DIR --bench
//! ```

use std::path::Path;
use std::time::{Duration, Instant};

use cairn_wasm::Derivation;
use cairn_wasm::cases::{Group, budget_group, limits_group, server_groups};

/// Timed runs per measure; the median is reported.
const RUNS: usize = 5;

/// A field of `/proc/self/status` in KiB, such as `VmHWM`; none off Linux.
fn status_kib(field: &str) -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with(field))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

/// The median of `RUNS` timings of `work`, in milliseconds.
fn median_ms(mut work: impl FnMut()) -> f64 {
    let mut times: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            work();
            started.elapsed()
        })
        .collect();
    times.sort();
    times[RUNS / 2].as_secs_f64() * 1000.0
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

/// The native half of the benchmark, over the document at the limits.
fn bench(document: &str) -> serde_json::Value {
    let derivation = Derivation::new(document).unwrap_or_else(|error| panic!("{error}"));
    let level =
        r#"{"projection":"level","shown":["group","action","deliverable","decision","milestone"]}"#;
    serde_json::json!({
        "runs": RUNS,
        "document_bytes": document.len(),
        "derived_bytes": derivation.derived().len(),
        "read_and_derive_ms": median_ms(|| drop(Derivation::new(document))),
        "derived_to_json_ms": median_ms(|| drop(derivation.derived())),
        "level_ms": median_ms(|| drop(derivation.project(level))),
        "peak_resident_kib": status_kib("VmHWM"),
    })
}

fn main() {
    let mut arguments = std::env::args().skip(1);
    let Some(out) = arguments.next() else {
        panic!("usage: cairn-wasm-cases DIR [--bench]");
    };
    let benchmark = arguments.next().is_some_and(|flag| flag == "--bench");
    let out = Path::new(&out);
    std::fs::create_dir_all(out).unwrap_or_else(|error| panic!("{}: {error}", out.display()));
    let mut groups: Vec<Group> = server_groups();
    groups.extend([500, 2_000].map(budget_group));
    groups.push(limits_group());
    let mut index = Vec::new();
    for (number, group) in groups.iter().enumerate() {
        let file = format!("group-{number:02}.json");
        write(
            &out.join(&file),
            &serde_json::to_string(group).unwrap_or_default(),
        );
        index.push(
            serde_json::json!({ "label": group.label, "file": file, "cases": group.cases.len() }),
        );
    }
    write(
        &out.join("index.json"),
        &serde_json::Value::from(index).to_string(),
    );
    if benchmark {
        let document = groups
            .last()
            .and_then(|group| group.document.as_deref())
            .unwrap_or_default();
        write(&out.join("bench-native.json"), &bench(document).to_string());
    }
    println!(
        "cases: {} groups written to {}",
        groups.len(),
        out.display()
    );
}
