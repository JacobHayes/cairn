//! The removal read (A18): the exact removal of a node as the graph stands, with the
//! references from outside its subtree that a patch removing it must also rewrite.
//!
//! Cost at `node_count_max`: one removal index (O(n log n + edges)), then one pass over the
//! nodes outside the subtree, each reading at most 16 condition clauses, 2 x 64 rule sources,
//! and its message drafts, plus the snoozes.

use std::collections::BTreeSet;

use cairn_schema::limits::PAGE_ITEM_COUNT_MAX;
use cairn_schema::{
    AnswerSpec, Cursor, DateSource, KeyRefs, Node, NodeKey, OutsideReference, Payload,
    ReferenceField, RemovalRead, ResourceContent, Segment, SnoozeTarget,
};

use super::rows::{count, page};
use super::{DerivedJourney, ProjectionError};
use crate::edit::full_removals;

impl DerivedJourney<'_> {
    /// A18: the removal of `key` and a page of the outside references to what it removes.
    ///
    /// # Errors
    ///
    /// When the node is not in the journey.
    pub fn removal(&self, key: &NodeKey, cursor: Cursor) -> Result<RemovalRead, ProjectionError> {
        self.known(key)?;
        let document = self.graph.document();
        let Some(removal) = full_removals(document, std::slice::from_ref(key)).pop() else {
            unreachable!("one removal per node asked for")
        };
        let inside: BTreeSet<&NodeKey> = removal.nodes().collect();
        let mut found = BTreeSet::new();
        let mut name = |holder: &NodeKey, field: ReferenceField, target: &NodeKey| {
            if inside.contains(target) {
                found.insert(OutsideReference {
                    holder: holder.clone(),
                    field,
                    target: target.clone(),
                });
            }
        };
        for node in document
            .nodes
            .values()
            .filter(|node| !inside.contains(&node.key))
        {
            references(node, |field, target| name(&node.key, field, target));
        }
        for (holder, until) in &document.state.snoozes {
            if let SnoozeTarget::Node(target) = until
                && !inside.contains(holder)
            {
                name(holder, ReferenceField::Snooze, target);
            }
        }
        let found: Vec<OutsideReference> = found.into_iter().collect();
        let (kept, next) = page(&found, cursor, PAGE_ITEM_COUNT_MAX);
        Ok(RemovalRead {
            removal,
            outside_references: kept,
            next,
            total: count(found.len()),
        })
    }
}

/// Every node a node's own fields name, with the field that names it.
fn references(node: &Node<KeyRefs>, mut name: impl FnMut(ReferenceField, &NodeKey)) {
    if let Some(condition) = &node.relevant_when {
        for decision in condition.decisions() {
            name(ReferenceField::RelevantWhen, decision);
        }
    }
    let rules = [
        (ReferenceField::DueBy, node.due_by.as_ref()),
        (ReferenceField::NotBefore, node.not_before.as_ref()),
    ];
    for (field, rule) in rules {
        for source in rule.into_iter().flat_map(|rule| rule.sources.as_set()) {
            if let DateSource::Node(source) = source {
                name(field, source);
            }
        }
    }
    match &node.payload {
        Payload::Group(group) => {
            let bounds = [
                (ReferenceField::OpensAt, group.opens_at.as_ref()),
                (ReferenceField::ClosesAt, group.closes_at.as_ref()),
            ];
            for (field, bound) in bounds {
                if let Some(bound) = bound {
                    name(field, bound);
                }
            }
        }
        Payload::Decision(decision) => {
            if let AnswerSpec::Date {
                feeds_milestone: Some(milestone),
            } = &decision.answer
            {
                name(ReferenceField::FeedsMilestone, milestone);
            }
        }
        Payload::Deliverable(_) | Payload::Action(_) | Payload::Milestone(_) => {}
    }
    for resource in &node.resources {
        if let ResourceContent::MessageDraft(template) = &resource.content {
            for segment in template.segments() {
                if let Segment::Answer(decision) = segment {
                    name(ReferenceField::MessageDraft, decision);
                }
            }
        }
    }
}
