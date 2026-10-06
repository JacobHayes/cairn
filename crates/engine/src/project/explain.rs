//! Explanation paging (ARCHITECTURE, Read path: server responses carry each explanation list's
//! largest entries up to `explanation_entry_count_max` and a total, and page the rest) and
//! J4's history, per node or per journey, grouped by patch.
//!
//! Cost at `node_count_max`: an explanation list is listed in full and sorted (at most 2,000
//! entries, pass 6) and a page cut from it; history reads each event given once (the nodes it
//! names off its delta) and cuts a page of `page_item_count_max` events.

use cairn_schema::limits::{EXPLANATION_ENTRY_COUNT_MAX, PAGE_ITEM_COUNT_MAX};
use cairn_schema::{
    Cursor, Event, ExplainedField, ExplanationPage, HistoryPage, NodeKey, PatchEvents,
};

use super::rows::{count, page};
use super::{DerivedJourney, ProjectionError};

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
        let entries = match field {
            ExplainedField::Gravity => priority.gravity_from(key),
            ExplainedField::Leverage => priority.leverage_from(key),
        };
        let (kept, next) = page(&entries, cursor, EXPLANATION_ENTRY_COUNT_MAX);
        Ok(ExplanationPage {
            node: key.clone(),
            field,
            entries: kept,
            next,
            total: count(entries.len()),
        })
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
