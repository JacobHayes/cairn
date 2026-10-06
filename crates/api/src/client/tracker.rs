//! H6's subscription tracking, as logic that knows no transport: a view holds the revision
//! it fetched of each domain it shows, and a tick asks it to refetch only when the tick is
//! newer than what it holds and than a refetch it already asked for. The current
//! revisions a stream starts with, again on every reconnect, ask for nothing the view
//! already has, so a view never misses a commit and never refetches twice for one. A
//! refetch that fails, or a stream that drops, forgets what was asked for, so the next tick
//! (a reconnect's current revisions among them) asks again: a fault delays a view, never
//! leaves it stale once the fault is over.

use std::collections::BTreeMap;

use cairn_schema::{Revision, RevisionOf};

use crate::wire::Tick;

/// What a view holds, and what it has asked to refetch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tracker {
    held: BTreeMap<RevisionOf, Revision>,
    wanted: BTreeMap<RevisionOf, Revision>,
}

impl Tracker {
    /// A view holding nothing yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The view now holds `of` at `revision`, from a fetch: never older than it held.
    pub fn fetched(&mut self, of: &RevisionOf, revision: Revision) {
        let held = self.held.entry(of.clone()).or_insert(revision);
        *held = (*held).max(revision);
        let held = *held;
        if self.wanted.get(of).is_some_and(|wanted| *wanted <= held) {
            self.wanted.remove(of);
        }
    }

    /// The refetch a tick asked for of `of` failed: the next tick at that revision asks
    /// for it again.
    pub fn refetch_failed(&mut self, of: &RevisionOf) {
        self.wanted.remove(of);
    }

    /// The stream was opened again: every refetch asked for and not yet made is forgotten,
    /// and the current revisions the new stream starts with ask for whatever is newer than
    /// what the view holds.
    pub fn reopened(&mut self) {
        self.wanted.clear();
    }

    /// The revision the view holds of `of`, if it holds it.
    #[must_use]
    pub fn held(&self, of: &RevisionOf) -> Option<Revision> {
        self.held.get(of).copied()
    }

    /// H6: whether `tick` asks the view to refetch `tick.of`: it is newer than what the view
    /// holds and than any refetch already asked for.
    pub fn ticked(&mut self, tick: &Tick) -> bool {
        let known = [self.held.get(&tick.of), self.wanted.get(&tick.of)]
            .into_iter()
            .flatten()
            .max()
            .copied();
        if known.is_some_and(|known| known >= tick.revision) {
            return false;
        }
        self.wanted.insert(tick.of.clone(), tick.revision);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_schema::Domain;

    fn tick(number: u32) -> Tick {
        Tick {
            of: RevisionOf::Domain(Domain::Deployment),
            revision: number.try_into().unwrap(),
        }
    }

    #[test]
    fn only_a_newer_tick_asks_for_a_refetch_and_only_once() {
        let mut tracker = Tracker::new();
        let of = tick(0).of;
        assert!(tracker.ticked(&tick(2)), "nothing held: fetch it");
        assert!(!tracker.ticked(&tick(2)), "already asked");
        tracker.fetched(&of, 3.try_into().unwrap());
        assert!(
            !tracker.ticked(&tick(2)),
            "a reconnect's current revision, older"
        );
        assert!(!tracker.ticked(&tick(3)), "held");
        assert!(tracker.ticked(&tick(4)));
        tracker.fetched(&of, 2.try_into().unwrap());
        assert_eq!(
            tracker.held(&of),
            Some(3.try_into().unwrap()),
            "never older"
        );
        assert!(!tracker.ticked(&tick(4)), "still asked for");
        tracker.fetched(&of, 4.try_into().unwrap());
        assert!(tracker.ticked(&tick(5)));
    }

    #[test]
    fn a_failed_refetch_or_a_reopened_stream_asks_again() {
        let mut tracker = Tracker::new();
        let of = tick(0).of;
        tracker.fetched(&of, 4.try_into().unwrap());
        assert!(tracker.ticked(&tick(5)));
        tracker.refetch_failed(&of);
        assert!(
            tracker.ticked(&tick(5)),
            "the reconnect's current revision asks again"
        );
        tracker.reopened();
        assert!(tracker.ticked(&tick(5)), "a reopened stream asks again");
        assert_eq!(tracker.held(&of), Some(4.try_into().unwrap()));
    }
}
