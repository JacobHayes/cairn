//! D5: a journey is stalled when its acting frontier is empty while in-scope, effectively
//! unfinished work remains (open, not a group, not closed). The diagnostic says what it waits
//! on, as data:
//!
//! - each frontier node held off the acting frontier: by a snooze that holds, with its target
//!   (`unsnooze` is offered), or, for an `auto_reach` milestone, by its date still ahead
//!   (`confirm reached` is offered);
//! - each gating node: the unfinished target of such a node snooze, which lifts the snooze
//!   when it satisfies dependencies; a surface follows it to its own blocking.
//!
//! Nothing else needs naming: every blocked node waits on a dependency that is itself open,
//! and following open dependencies (acyclic) ends at a node that is not blocked, which is on
//! the frontier unless it is closed. So when the acting frontier is empty, the held frontier
//! nodes are where every chain of open dependencies ends. `all_blocked` (D5: shown as
//! "blocked") is set when every remaining node is blocked, which that argument says a valid
//! journey never reaches; the property tests hold it to that.
//!
//! Deterministic and bounded: the held nodes in key order, then the gating nodes in key
//! order, each once; O(nodes).

use std::collections::BTreeSet;

use cairn_schema::{NodeKey, NodeKind, SnoozeTarget, StallCause, Stalled};

use super::{Blocking, Sweep};

/// D5: the stalled diagnostic, or none when the acting frontier is not empty or no work
/// remains.
pub(super) fn diagnose(blocking: &Blocking, sweep: &Sweep<'_>) -> Option<Stalled> {
    if !blocking.acting_frontier.is_empty() {
        return None;
    }
    let remaining: Vec<&NodeKey> = blocking
        .keys
        .iter()
        .zip(&blocking.flags)
        .filter(|(key, flags)| {
            let group = sweep
                .graph
                .node(key)
                .is_none_or(|node| node.kind() == NodeKind::Group);
            !group && !flags.closed && sweep.early.relevance.in_scope(key)
        })
        .map(|(key, _)| key)
        .collect();
    if remaining.is_empty() {
        return None;
    }
    let all_blocked = remaining.iter().all(|key| blocking.blocked(key));
    let mut waiting_on = Vec::new();
    let mut gates = BTreeSet::new();
    for key in &blocking.frontier {
        if let Some(until) = blocking.snoozed(key) {
            if let SnoozeTarget::Node(target) = until {
                gates.insert(target.clone());
            }
            waiting_on.push(StallCause::Snooze {
                node: key.clone(),
                until: until.clone(),
            });
        } else if let Some(date) = sweep.reach_ahead(key) {
            waiting_on.push(StallCause::AutoReach {
                node: key.clone(),
                date,
            });
        }
    }
    // A target on the frontier is listed by its own hold.
    let held: BTreeSet<&NodeKey> = blocking.frontier.iter().collect();
    waiting_on.extend(
        gates
            .into_iter()
            .filter(|target| !held.contains(target))
            .map(StallCause::Gate),
    );
    assert!(
        all_blocked || !waiting_on.is_empty(),
        "a stalled journey with an open node it can reach names what it waits on"
    );
    Some(Stalled {
        waiting_on,
        all_blocked,
    })
}
