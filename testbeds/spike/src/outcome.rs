//! Judging a finished run: the invariants over what the clients were told and what the
//! server's log holds, reported through patina's verdict channel.

use std::collections::BTreeMap;
use std::process::ExitCode;
use std::time::Duration;

use patina_dst::VerdictKind;

use crate::RequestId;
use crate::client::Report;

/// What a run produced, ready to judge.
pub struct Outcome {
    acknowledged: Vec<(RequestId, u64)>,
    log: Vec<Result<RequestId, String>>,
    attempts: u64,
    deduplicated: u64,
    /// Virtual time from the first client request to the log read's answer.
    elapsed: Duration,
}

impl Outcome {
    pub fn new(
        reports: &[Report],
        log: &str,
        log_attempts: u64,
        deduplicated: u64,
        elapsed: Duration,
    ) -> Self {
        let mut acknowledged: Vec<_> = reports
            .iter()
            .flat_map(|report| report.acknowledged.iter().copied())
            .collect();
        acknowledged.sort_unstable();
        Self {
            acknowledged,
            log: log.lines().map(str::parse).collect(),
            attempts: reports.iter().map(|report| report.attempts).sum::<u64>() + log_attempts,
            deduplicated,
            elapsed,
        }
    }

    /// Every invariant the run broke, as (label, detail) pairs; empty when it held.
    fn violations(&self) -> Vec<(&'static str, String)> {
        let mut violations = Vec::new();
        let mut applied: BTreeMap<RequestId, u64> = BTreeMap::new();
        for (index, entry) in self.log.iter().enumerate() {
            match entry {
                Ok(id) => *applied.entry(*id).or_default() += 1,
                Err(error) => {
                    violations.push(("log-well-formed", format!("line {index}: {error}")));
                }
            }
        }
        for (id, times) in &applied {
            if *times > 1 {
                violations.push(("applied-once", format!("{id} applied {times} times")));
            }
        }
        for &(id, revision) in &self.acknowledged {
            let at_revision = usize::try_from(revision)
                .ok()
                .and_then(|revision| revision.checked_sub(1))
                .and_then(|index| self.log.get(index));
            if at_revision != Some(&Ok(id)) {
                violations.push((
                    "acknowledged-visible",
                    format!("{id} acknowledged at revision {revision}, log has {at_revision:?}"),
                ));
            }
        }
        for id in applied.keys() {
            if self
                .acknowledged
                .binary_search_by_key(id, |&(acked, _)| acked)
                .is_err()
            {
                violations.push((
                    "no-phantom-entries",
                    format!("{id} is in the log unacknowledged"),
                ));
            }
        }
        violations
    }

    /// Reports the run's verdicts and returns its exit code: a `pass` under `spike-outcome`
    /// carrying the counts, or one `violation` per broken invariant.
    pub fn judge(&self, flavor: &str, requests: u32) -> ExitCode {
        // One attempt per request plus the final log read, on a run with nothing to retry.
        let attempts_minimum = u64::from(requests) + 1;
        let retries = self.attempts.saturating_sub(attempts_minimum);
        let detail = format!(
            "flavor={flavor} requests={requests} acknowledged={} log={} attempts={} retries={retries} \
             deduplicated={} elapsed_us={} ids={:016x} order={:016x}",
            self.acknowledged.len(),
            self.log.len(),
            self.attempts,
            self.deduplicated,
            self.elapsed.as_micros(),
            fnv(self.acknowledged.iter().map(|(id, _)| id.to_string())),
            fnv(self.log.iter().map(|entry| format!("{entry:?}"))),
        );
        let violations = self.violations();
        if violations.is_empty() {
            patina_dst::verdict(VerdictKind::Pass, "spike-outcome", &detail);
            eprintln!("SPIKE_RESULT {detail}");
            return ExitCode::SUCCESS;
        }
        for (label, violation) in &violations {
            patina_dst::verdict(VerdictKind::Violation, label, violation);
            eprintln!("SPIKE_VIOLATION {label} {violation}");
        }
        eprintln!("SPIKE_RESULT {detail}");
        ExitCode::FAILURE
    }
}

/// FNV-1a over `items`, each followed by a newline: a stable digest for the result line.
fn fnv(items: impl Iterator<Item = String>) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for item in items {
        for byte in item.bytes().chain(std::iter::once(b'\n')) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(client: u32, sequence: u32) -> RequestId {
        RequestId { client, sequence }
    }

    fn outcome(acknowledged: &[(RequestId, u64)], log: &str) -> Outcome {
        let report = Report {
            acknowledged: acknowledged.to_vec(),
            attempts: 0,
        };
        Outcome::new(&[report], log, 0, 0, Duration::ZERO)
    }

    struct Case {
        name: &'static str,
        acknowledged: &'static [(u32, u64)],
        log: &'static str,
        violated: &'static [&'static str],
    }

    /// Each invariant fires on the log shape that breaks it, and only then. Acknowledgements
    /// are (client, revision) pairs for sequence 0.
    #[test]
    fn violations_name_the_broken_invariants() {
        let cases = [
            Case {
                name: "clean",
                acknowledged: &[(0, 1), (1, 2)],
                log: "0-0\n1-0\n",
                violated: &[],
            },
            Case {
                name: "applied twice",
                acknowledged: &[(0, 1), (1, 3)],
                log: "0-0\n0-0\n1-0\n",
                violated: &["applied-once"],
            },
            Case {
                name: "acknowledged at a revision holding another id",
                acknowledged: &[(0, 2), (1, 1)],
                log: "0-0\n1-0\n",
                violated: &["acknowledged-visible", "acknowledged-visible"],
            },
            Case {
                name: "acknowledged but missing",
                acknowledged: &[(0, 1), (1, 2)],
                log: "0-0\n",
                violated: &["acknowledged-visible"],
            },
            Case {
                name: "in the log unacknowledged",
                acknowledged: &[(0, 1)],
                log: "0-0\n2-0\n",
                violated: &["no-phantom-entries"],
            },
            Case {
                name: "unparseable line",
                acknowledged: &[(0, 1)],
                log: "0-0\nnot-an-id\n",
                violated: &["log-well-formed"],
            },
        ];
        for case in cases {
            let acknowledged: Vec<_> = case
                .acknowledged
                .iter()
                .map(|&(client, revision)| (id(client, 0), revision))
                .collect();
            let labels: Vec<_> = outcome(&acknowledged, case.log)
                .violations()
                .into_iter()
                .map(|(label, _)| label)
                .collect();
            assert_eq!(labels, case.violated, "{}", case.name);
        }
    }

    #[test]
    fn request_ids_round_trip_through_text() {
        let original = id(3, 17);
        assert_eq!(original.to_string().parse::<RequestId>(), Ok(original));
        for text in ["", "3", "3-", "-17", "a-1", "1-2-3"] {
            assert!(text.parse::<RequestId>().is_err(), "{text:?} parsed");
        }
    }
}
