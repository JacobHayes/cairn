//! The touched set (PRD glossary; H5): everything a patch or event writes or removes, by key
//! and field, including a removed node's subtree and every incident edge. A stale answer
//! carries the set the commits since the revision a write named touched, so its writer knows
//! what to read again.
//!
//! A touched set is only ever read off writes ([`crate::Write::keys`]): an event's delta, or
//! the change set the engine's apply builds. What a patch touches depends on what it finds
//! (removing a role clears its fill, reopening a node its bypass), so nothing predicts it
//! from the mutations alone; a caller reads the patch's own off applying it
//! (the engine's `touched`).

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::record::RecordKey;

/// A set of record addresses.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct TouchedSet(BTreeSet<RecordKey>);

impl TouchedSet {
    /// The addresses, sorted.
    #[must_use]
    pub fn as_set(&self) -> &BTreeSet<RecordKey> {
        &self.0
    }

    /// Adds an address.
    pub fn insert(&mut self, key: RecordKey) {
        self.0.insert(key);
    }

    /// Adds every address of another set.
    pub fn extend(&mut self, other: TouchedSet) {
        self.0.extend(other.0);
    }

    /// True when nothing is touched.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<RecordKey> for TouchedSet {
    fn from_iter<I: IntoIterator<Item = RecordKey>>(keys: I) -> Self {
        Self(keys.into_iter().collect())
    }
}
