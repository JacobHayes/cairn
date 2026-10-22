//! The checklist count (D3, G5): how many of a node's checklist items are ticked, one rule
//! for node detail and every row.
//!
//! Cost at the limits: one walk of the node's items (at most `checklist_item_count_per_node_max`)
//! with a set lookup into its ticks.

use cairn_schema::{ChecklistCount, KeyRefs, Node};

use crate::graph::Document;

/// D3, G5: the node's ticked items of its checklist, or none when it has no items. Counts by
/// the items, so a tick naming no item (which validation refuses) is never over-counted.
#[must_use]
pub fn checklist_count(document: &Document, node: &Node<KeyRefs>) -> Option<ChecklistCount> {
    let checklist = node.payload.checklist().filter(|list| !list.is_empty())?;
    let ticks = document.state.ticks.get(&node.key);
    let ticked = checklist
        .ids()
        .filter(|id| ticks.is_some_and(|ticks| ticks.contains(*id)))
        .count();
    Some(ChecklistCount {
        ticked: u32::try_from(ticked).unwrap_or(u32::MAX),
        total: u32::try_from(checklist.len()).unwrap_or(u32::MAX),
    })
}
