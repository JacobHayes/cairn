//! Explanation paging (ARCHITECTURE, Read path: server responses carry each explanation list's
//! largest entries up to `explanation_entry_count_max` and a total, and page the rest) and
//! J4's history, per node or per journey, grouped by patch.
//!
//! Cost at `node_count_max`: an explanation list is listed in full and sorted (at most 2,000
//! entries, pass 6) and a page cut from it; a still-waiting list runs one unlocks
//! simulation and reads each direct dependent's effective dependencies once (at most 2,000
//! dependents, each about 1,300 edges); history reads each event given once (the nodes it
//! names off its delta) and cuts a page of `page_item_count_max` events.

use cairn_schema::limits::{EXPLANATION_ENTRY_COUNT_MAX, PAGE_ITEM_COUNT_MAX};
use std::collections::BTreeSet;

use cairn_schema::{
    Cursor, Event, ExplainedField, ExplanationPage, HeldDependent, HistoryPage, NodeKey,
    PatchEvents,
};

use super::rows::{count, page};
use super::{DerivedJourney, ProjectionError};
use crate::derive::dependencies::{EdgeClass, EdgeSet, EdgeSource, Instant, Point};

impl DerivedJourney<'_> {
    /// A page of the node's explanation list for `field`, largest first, from the cursor; the
    /// first page is the list node detail carries.
    ///
    /// # Errors
    ///
    /// When the node is not in the journey.
    pub fn explanations(
        &self,
        key: &NodeKey,
        field: ExplainedField,
        cursor: Cursor,
    ) -> Result<ExplanationPage, ProjectionError> {
        self.known(key)?;
        let priority = self.derived.priority();
        let (entries, held) = match field {
            ExplainedField::Gravity => (priority.gravity_from(key), Vec::new()),
            ExplainedField::Unlocks => (priority.unlocks_from(key), Vec::new()),
            ExplainedField::StillWaiting => (Vec::new(), self.still_waiting(key)?),
        };
        let (kept, next_entries) = page(&entries, cursor, EXPLANATION_ENTRY_COUNT_MAX);
        let (kept_held, next_held) = page(&held, cursor, EXPLANATION_ENTRY_COUNT_MAX);
        Ok(ExplanationPage {
            node: key.clone(),
            field,
            entries: kept,
            held: kept_held,
            next: next_entries.or(next_held),
            total: count(entries.len() + held.len()),
        })
    }

    /// C8, Priority: Unlocks: the node's direct dependents that completing it would not yet
    /// free, each with what else it waits on (its unsatisfied requirements, condition gates,
    /// and stage openings, its own and inherited), largest weight first, then by key. The
    /// complement of `unlocks_from` among the node's open, in-scope dependents: the ones
    /// the unlocks do not count. Empty for a node outside the rank normalization set (closed,
    /// not relevant, or a group), which completing frees nothing.
    ///
    /// # Errors
    ///
    /// When the node is not in the journey.
    pub fn still_waiting(&self, key: &NodeKey) -> Result<Vec<HeldDependent>, ProjectionError> {
        self.known(key)?;
        let derived = self.derived;
        let dependencies = derived.dependencies();
        let Some(index) = dependencies
            .node_index(key)
            .filter(|_| derived.priority().in_normalization_set(key))
        else {
            return Ok(Vec::new());
        };
        let finished = derived.finished_by(self.graph, key);
        let freed: BTreeSet<NodeKey> = derived
            .priority()
            .unlocks_from(key)
            .into_iter()
            .map(|freed| freed.node)
            .collect();
        // The nodes whose own requirement, condition gate, or stage opening waits for this
        // node's finish; what an ancestor's holds reaches a descendant through the ancestor.
        let dependents: BTreeSet<&NodeKey> = dependencies
            .waited_by(Instant::new(index, Point::Finish), EdgeSet::Pruned)
            .filter(|edge| edge.class == EdgeClass::Gate)
            .filter(|edge| {
                matches!(
                    edge.source,
                    EdgeSource::Explicit | EdgeSource::Condition | EdgeSource::StageOpening
                )
            })
            .filter_map(|edge| dependencies.key(edge.dependent.node))
            .collect();
        let mut held: Vec<HeldDependent> = dependents
            .into_iter()
            .filter(|dependent| *dependent != key && !freed.contains(*dependent))
            .filter(|dependent| {
                derived.relevance().in_scope(dependent) && !derived.blocking().closed(dependent)
            })
            .filter_map(|dependent| {
                let also_waits_on = derived.held_besides(dependent, &finished);
                (!also_waits_on.is_empty()).then(|| HeldDependent {
                    node: dependent.clone(),
                    also_waits_on,
                })
            })
            .collect();
        held.sort_by(|a, b| {
            let weight = |held: &HeldDependent| self.node(&held.node).effective_weight().get();
            weight(b).cmp(&weight(a)).then_with(|| a.node.cmp(&b.node))
        });
        Ok(held)
    }
}

/// J4: a page of a journey's history, or of one node's (the events naming it), from the
/// cursor: at most `page_item_count_max` events in log order, grouped by patch. `events` is
/// the journey's log in order; a patch's events are contiguous in it.
#[must_use]
pub fn history(events: &[Event], node: Option<&NodeKey>, cursor: Cursor) -> HistoryPage {
    let named: Vec<&Event> = events
        .iter()
        .filter(|event| node.is_none_or(|node| event.nodes().contains(node)))
        .collect();
    let (kept, next) = page(&named, cursor, PAGE_ITEM_COUNT_MAX);
    let mut patches: Vec<PatchEvents> = Vec::new();
    for event in kept {
        match patches.last_mut() {
            Some(group) if group.patch_id == event.patch_id => group.events.push(event.clone()),
            Some(_) | None => patches.push(PatchEvents {
                patch_id: event.patch_id.clone(),
                events: vec![event.clone()],
            }),
        }
    }
    HistoryPage {
        patches,
        next,
        total: count(named.len()),
    }
}
