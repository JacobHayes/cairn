//! Insertions (PRD Invariants: insertions; B13): every member of an insertion is a node of
//! the graph and a member of no other insertion, every insertion has a member, the roles and
//! kinds it maps onto exist, and in a journey a node has `from_segment` provenance exactly
//! when it is a member. What only the segment version can tell (its keys, the cardinality of
//! what it maps) is checked where the insertion is made. Cost at the limits: one pass over
//! the insertions' members and one over the journey's node states, each one lookup.

use std::collections::BTreeMap;

use cairn_schema::{InsertionKey, KindKey, NodeKey, Provenance, Subject, Violation, ViolationCode};

use super::{GraphCheck, at_node, violation};

/// Runs the insertion checks.
pub(super) fn check(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    let document = check.document;
    let mut holders: BTreeMap<&NodeKey, &InsertionKey> = BTreeMap::new();
    for insertion in document.insertions.values() {
        let about = |message: String| {
            let mut found = violation(ViolationCode::InsertionInvalid, message);
            found.at.subject = Some(Subject::Insertion(insertion.key.clone()));
            found
        };
        if insertion.nodes.is_empty() {
            out.push(about("an insertion has at least one member".to_owned()));
        }
        for member in insertion.nodes.keys() {
            if document.nodes.get(member).is_none() {
                out.push(about(format!("the member {member} is not in the graph")));
            } else if let Some(other) = holders.insert(member, &insertion.key) {
                out.push(at_node(
                    check.tree,
                    member,
                    ViolationCode::InsertionInvalid,
                    format!(
                        "the node is a member of two insertions, {other} and {}",
                        insertion.key
                    ),
                ));
            }
        }
        for role in insertion.roles.values() {
            if document.roles.get(role).is_none() {
                out.push(about(format!(
                    "the role {role} it maps onto is not in the graph"
                )));
            }
        }
        for kind in insertion.kinds.values() {
            if *kind != KindKey::owner() && document.participation_kinds.get(kind).is_none() {
                out.push(about(format!(
                    "the participation kind {kind} it maps onto is not in the graph"
                )));
            }
        }
    }
    if check.journey {
        provenance(check, &holders, out);
    }
}

/// In a journey, a node is `from_segment` exactly when it is a member.
fn provenance(
    check: &GraphCheck<'_>,
    holders: &BTreeMap<&NodeKey, &InsertionKey>,
    out: &mut Vec<Violation>,
) {
    for (key, stored) in &check.document.state.nodes {
        let segment = stored.provenance == Provenance::FromSegment;
        let member = holders.contains_key(key);
        if segment != member && check.document.nodes.get(key).is_some() {
            let message = if segment {
                "a node copied in from a segment is a member of an insertion (B13)"
            } else {
                "a member of an insertion has from_segment provenance (B13)"
            };
            out.push(at_node(
                check.tree,
                key,
                ViolationCode::InsertionInvalid,
                message,
            ));
        }
    }
}
