//! H6's subscription tracking, as logic that knows no transport (the TypeScript client has
//! its own, and both run the cases in `web/client/src/tracker-cases.json`): a view holds the revision
//! it fetched of each domain it shows, and a tick asks it to refetch only when the tick is
//! newer than what it holds and than a refetch it already asked for. The current
//! revisions a stream starts with, again on every reconnect, ask for nothing the view
//! already has, so a view never misses a commit and never refetches twice for one. A
//! refetch that fails, or a stream that drops, forgets what was asked for, so the next tick
//! (a reconnect's current revisions among them) asks again: a fault delays a view, never
//! leaves it stale once the fault is over.

use std::collections::BTreeMap;

use cairn_schema::{Revision, RevisionOf};

use cairn_service::wire::Tick;

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
    use serde::Deserialize;

    /// A named run of moves with what each must show: the cases the TypeScript client runs
    /// too (`web/client/src/tracker-cases.json`), so the two cannot drift.
    #[derive(Deserialize)]
    struct Case {
        name: String,
        steps: Vec<Step>,
    }

    #[derive(Deserialize)]
    #[serde(tag = "op", rename_all = "snake_case")]
    enum Step {
        Fetched {
            of: Option<RevisionOf>,
            revision: u32,
        },
        Ticked {
            of: Option<RevisionOf>,
            revision: u32,
            asks: bool,
        },
        RefetchFailed {
            of: Option<RevisionOf>,
        },
        Reopened,
        Held {
            of: Option<RevisionOf>,
            holds: Option<u32>,
        },
    }

    fn deployment() -> RevisionOf {
        RevisionOf::Domain(Domain::Deployment)
    }

    #[test]
    fn the_shared_cases_hold() {
        let cases: Vec<Case> = serde_json::from_str(include_str!(
            "../../../../web/client/src/tracker-cases.json"
        ))
        .unwrap();
        assert!(!cases.is_empty());
        for case in cases {
            let mut tracker = Tracker::new();
            for (index, step) in case.steps.into_iter().enumerate() {
                let at = format!("{}, step {index}", case.name);
                match step {
                    Step::Fetched { of, revision } => {
                        tracker
                            .fetched(&of.unwrap_or_else(deployment), revision.try_into().unwrap());
                    }
                    Step::Ticked { of, revision, asks } => {
                        let tick = Tick {
                            of: of.unwrap_or_else(deployment),
                            revision: revision.try_into().unwrap(),
                        };
                        assert_eq!(tracker.ticked(&tick), asks, "{at}");
                    }
                    Step::RefetchFailed { of } => {
                        tracker.refetch_failed(&of.unwrap_or_else(deployment));
                    }
                    Step::Reopened => tracker.reopened(),
                    Step::Held { of, holds } => {
                        let held = tracker.held(&of.unwrap_or_else(deployment));
                        assert_eq!(held, holds.map(|number| number.try_into().unwrap()), "{at}");
                    }
                }
            }
        }
    }
}
