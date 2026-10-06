//! What a restarted incarnation learns before and after it opens the store: the step that
//! was in flight when the process stopped, the last commit point the store reported for it,
//! and whether the crash left a torn tail on Turso's logical log, which the store's open must
//! discard (Turso's frame-level atomicity: a frame whose trailer is missing is not replayed).
//!
//! The coverage oracles here fire only in a restarted incarnation, which only a crash-restart
//! makes, and `cargo patina campaign` draws no crash (DECISIONS.md, 6.2, patina). Their labels
//! are constants rather than literals, so they stay out of the link-time site table a
//! campaign gates on; the crash sweep in `sim.sh` gates on them instead.

use std::collections::BTreeSet;
use std::path::Path;

use cairn_service::Written;

use crate::audit::Audit;
use crate::ledger::{Entry, Ledger, read_if_present};
use crate::world::{Stop, World, database, report_violation};

/// The crash sweep's oracles (`sim.sh` requires each to fire).
pub const RECOVERED: &str = "durability-restart-recovered-commits";
pub const INTERRUPTED_LOST: &str = "durability-crash-interrupted-commit-lost";
pub const INTERRUPTED_LANDED: &str = "durability-crash-interrupted-commit-landed";
pub const BETWEEN_STATE_AND_EVENTS: &str = "durability-crash-between-state-and-events";
pub const TORN_TAIL_DISCARDED: &str = "durability-torn-log-tail-discarded";

/// The last four bytes of a whole Turso logical-log frame (`END_MAGIC`, little-endian).
const FRAME_END: &[u8; 4] = b"MVTE";
/// The logical log's header length (`LOG_HDR_SIZE`).
const LOG_HEADER_LENGTH: usize = 56;

/// What the disk and the ledger say before the store opens.
pub struct Before {
    in_flight: Option<usize>,
    last_point: Option<String>,
    torn_tail: bool,
    log_bytes: usize,
    ledger_torn_bytes: u64,
}

impl Before {
    pub fn read(directory: &Path, ledger: &Ledger) -> Result<Before, Stop> {
        let mut in_flight = None;
        let mut last_point = None;
        for entry in ledger.entries() {
            match entry {
                Entry::Intent { index } => {
                    in_flight = Some(*index);
                    last_point = None;
                }
                Entry::Ack { .. } => in_flight = None,
                Entry::Point { point, .. } => last_point = Some(point.clone()),
                Entry::Plan { .. } | Entry::Restart => {}
            }
        }
        let mut log = database(directory).into_os_string();
        log.push("-log");
        let bytes = read_if_present(&log.into()).map_err(|error| Stop::Abort {
            label: "durability-store-files-readable",
            detail: error.to_string(),
        })?;
        Ok(Before {
            in_flight,
            last_point: last_point.filter(|_| in_flight.is_some()),
            torn_tail: bytes.as_deref().is_some_and(torn),
            log_bytes: bytes.as_ref().map_or(0, Vec::len),
            ledger_torn_bytes: ledger.torn_bytes,
        })
    }

    /// Fires the restart oracles once the restart audit has passed, and answers the step
    /// in flight with whether its commit landed.
    pub fn observe(&self, audit: &Audit) -> Option<(usize, bool)> {
        patina_dst::sometimes!(!audit.committed.is_empty(), RECOVERED);
        patina_dst::sometimes!(self.torn_tail, TORN_TAIL_DISCARDED);
        let in_flight = self.in_flight?;
        let landed = audit.committed.contains(&in_flight);
        patina_dst::sometimes!(!landed, INTERRUPTED_LOST);
        patina_dst::sometimes!(landed, INTERRUPTED_LANDED);
        let between = self.last_point.as_deref() == Some("BetweenStateAndEvents");
        patina_dst::sometimes!(between, BETWEEN_STATE_AND_EVENTS);
        eprintln!(
            "DURABILITY_RESTART in_flight={in_flight} landed={landed} last_point={} torn_tail={} log_bytes={} ledger_torn_bytes={}",
            self.last_point.as_deref().unwrap_or("none"),
            self.torn_tail,
            self.log_bytes,
            self.ledger_torn_bytes,
        );
        Some((in_flight, landed))
    }
}

/// Whether a logical log ends partway through its header or a frame.
fn torn(log: &[u8]) -> bool {
    match log.len() {
        0 | LOG_HEADER_LENGTH => false,
        length if length < LOG_HEADER_LENGTH => true,
        length => &log[length - FRAME_END.len()..] != FRAME_END,
    }
}

/// The steps the ledger records as acknowledged.
pub fn acknowledged(world: &World) -> BTreeSet<usize> {
    world
        .entries()
        .iter()
        .filter_map(|entry| match entry {
            Entry::Ack { index, .. } => Some(*index),
            _ => None,
        })
        .collect()
}

/// H5 across a crash: the step in flight, submitted again, is answered from its receipt
/// exactly when its commit landed before the crash.
pub fn judge_resubmission(
    in_flight: usize,
    index: usize,
    landed: bool,
    written: &Written,
) -> Result<(), Stop> {
    let from_receipt = matches!(written, Written::AlreadyApplied { .. });
    if index != in_flight || from_receipt == landed {
        return Ok(());
    }
    let detail = format!("step {index} landed={landed} answered_from_receipt={from_receipt}");
    report_violation(
        "durability-crash-resubmission-answered-from-receipt-when-landed",
        &detail,
    );
    Err(Stop::Violated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_log_is_torn_when_it_ends_inside_its_header_or_a_frame() {
        let header = vec![0u8; LOG_HEADER_LENGTH];
        let mut whole = header.clone();
        whole.extend_from_slice(b"frame bytes");
        whole.extend_from_slice(FRAME_END);
        let cases: [(&[u8], bool); 5] = [
            (&[], false),
            (&header, false),
            (&header[..10], true),
            (&whole, false),
            (&whole[..whole.len() - 1], true),
        ];
        for (log, expected) in cases {
            assert_eq!(torn(log), expected, "{} bytes", log.len());
        }
    }
}
