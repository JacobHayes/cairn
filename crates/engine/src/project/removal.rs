//! The removal read (A18): the exact removal of a node as the graph stands, with the
//! references from outside its subtree that a patch removing it must also rewrite.
//!
//! Cost at `node_count_max`: one removal index (O(n log n + edges)), then one pass over the
//! nodes outside the subtree, each reading at most 16 condition clauses, 2 x 64 rule sources,
//! and its message drafts, plus the snoozes.

use std::collections::BTreeSet;

use cairn_schema::limits::PAGE_ITEM_COUNT_MAX;
use cairn_schema::{Cursor, NodeKey, OutsideReference, RemovalRead};

use super::rows::{count, page};
use super::{DerivedJourney, ProjectionError};
use crate::cascade::outside_references;
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
        let found = outside_references(document, &inside);
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
