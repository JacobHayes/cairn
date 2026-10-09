//! A segment's graph (A21): a draft or version of a route of kind `segment` declares no
//! `final` milestone and no `default_owner`, and has at most one root node (a published
//! version exactly one, which publishing checks). The rules are the route's kind's, so they
//! are checked on every graph the patch wrote for a segment, however it was edited. Cost: one
//! pass over a segment's nodes and one tree.

use cairn_schema::{GraphId, Payload, RouteKind, Subject, ViolationCode};

use super::Check;
use crate::graph::Tree;
use crate::validate::{at_node, violation};

pub(super) fn check(check: &mut Check<'_, '_>) {
    mapped_cardinality(check);
    for id in super::graphs::written(check) {
        let candidate = &check.session.candidate;
        let (GraphId::RouteDraft(route) | GraphId::RouteVersion { route, .. }) = &id else {
            continue;
        };
        let segment = candidate
            .routes
            .get(route)
            .is_some_and(|held| held.header.kind == RouteKind::Segment);
        let Some(document) = candidate.graph(&id).filter(|_| segment) else {
            continue;
        };
        let tree = Tree::build(document);
        if document.default_owner.is_some() {
            check.violations.push(violation(
                ViolationCode::SegmentRule,
                "a segment declares no default_owner: the graph it is inserted into has one (A21)",
            ));
        }
        for node in document.nodes.values() {
            if matches!(&node.payload, Payload::Milestone(milestone) if milestone.is_final) {
                check.violations.push(at_node(
                    &tree,
                    &node.key,
                    ViolationCode::SegmentRule,
                    "a segment declares no final milestone: the graph it is inserted into has one (A21)",
                ));
            }
        }
        for root in tree.roots().iter().skip(1) {
            check.violations.push(at_node(
                &tree,
                root,
                ViolationCode::SegmentRule,
                "a segment has one root node; put the others under it (A21)",
            ));
        }
    }
}

/// B13: a role or kind an insertion maps onto keeps its cardinality, so a patch that flipped
/// one leaves no insertion in the graph it produces mapped onto it. Checked on the final
/// graph, so removing the insertion in the same patch is allowed.
fn mapped_cardinality(check: &mut Check<'_, '_>) {
    for id in super::graphs::written(check) {
        let Some(document) = check.session.candidate.graph(&id) else {
            continue;
        };
        for insertion in document.insertions.values() {
            let mapped = insertion
                .roles
                .values()
                .map(|role| Subject::Role(role.clone()))
                .chain(
                    insertion
                        .kinds
                        .values()
                        .map(|kind| Subject::Kind(kind.clone())),
                );
            for subject in mapped.filter(|s| check.session.cardinality_changed.contains(s)) {
                let mut found = violation(
                    ViolationCode::InsertionInvalid,
                    "a role or kind an insertion maps onto keeps its cardinality while the insertion is there (B13)",
                );
                found.at.subject = Some(subject);
                check.violations.push(found);
            }
        }
    }
}
