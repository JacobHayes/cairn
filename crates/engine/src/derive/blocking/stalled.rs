//! D5: a journey is stalled when its acting frontier is empty while in-scope, effectively
//! unfinished work remains (open, not a group, not closed). The diagnostic says what it waits
//! on, as data:
//!
//! - each frontier node held off the acting frontier: by a snooze that holds, with its target
//!   (`unsnooze` is offered), or, for an `auto_reach` milestone, by its date still ahead
//!   (`confirm reached` is offered); a node held through containers names the snooze of each
//!   container above it that holds, once, however many nodes it holds (B6);
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
//! Deterministic and bounded: the held nodes (and the containers holding others) in key
//! order, then the gating nodes in key order, each once; O(nodes log nodes).

use std::collections::{BTreeMap, BTreeSet};

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
    let causes = holds(blocking, sweep);
    let mut gates = BTreeSet::new();
    let mut waiting_on = Vec::new();
    for cause in causes.into_values() {
        if let StallCause::Snooze {
            until: SnoozeTarget::Node(target),
            ..
        } = &cause
        {
            gates.insert(target.clone());
        }
        waiting_on.push(cause);
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

/// Each hold on a frontier node, once, by the node it names: a held node, or the container
/// holding others (B6).
fn holds(blocking: &Blocking, sweep: &Sweep<'_>) -> BTreeMap<NodeKey, StallCause> {
    let snooze = |node: &NodeKey, until: &SnoozeTarget| StallCause::Snooze {
        node: node.clone(),
        until: until.clone(),
    };
    let mut causes = BTreeMap::new();
    for key in &blocking.frontier {
        let own = blocking.snoozed_own(key);
        let via = blocking.snoozed_via(key);
        if let Some(until) = own {
            causes.insert(key.clone(), snooze(key, until));
        }
        // Every container above it whose snooze holds, not only the nearest: lifting the
        // inner one alone leaves the outer one holding.
        let tree = sweep.graph.tree();
        let mut above = via;
        while let Some(container) = above {
            if let Some(until) = blocking.snoozed_own(container) {
                causes.insert(container.clone(), snooze(container, until));
            }
            above = tree.parent(container);
        }
        if let (None, None, Some(date)) = (own, via, sweep.reach_ahead(key)) {
            causes.insert(
                key.clone(),
                StallCause::AutoReach {
                    node: key.clone(),
                    date,
                },
            );
        }
    }
    causes
}
