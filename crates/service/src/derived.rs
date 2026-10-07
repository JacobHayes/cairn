//! Derived reads (ARCHITECTURE, Engine > Read path: derive; D3, D6): a journey loaded with
//! the deployment, derived at the caller's today with the caller's entities as the viewer,
//! and memoized per everything the derive reads, so a cached value is the value a fresh
//! derive would give.
//!
//! The memo key is the journey and its revision, the deployment revision (entities and
//! aliases, E6), today in the deployment's zone (A9), and the viewer; the time zone and rank
//! constants are the service's own settings, fixed for its life. A commit moves a revision and
//! midnight moves today, so neither ever meets an entry derived before it (D6: caching is
//! invisible). The memo holds at most [`MEMO_ENTRY_COUNT_MAX`] derivations and drops the least
//! recently read one first.
//!
//! Cost: a hit is one lock and a scan of at most `MEMO_ENTRY_COUNT_MAX` keys; a miss is one
//! graph validation and one derive (ARCHITECTURE, Read path: O(nodes + edges) per pass), run
//! outside the lock, so two readers missing at once each derive and the later insert wins.

use std::collections::{BTreeSet, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};

use cairn_engine::{Derived, Graph, derive};
use cairn_schema::{Date, Deployment, EntityKey, Journey, JourneyId, Revision};

/// How many derivations the memo keeps. A derivation at the limits holds a 2,000-node graph
/// and its derive, tens of MiB, so the bound keeps the worst case within a few hundred MiB
/// while a small team's open journeys, viewer by viewer, still hit (brief 4.8, Decision log).
pub(crate) const MEMO_ENTRY_COUNT_MAX: usize = 8;

/// What a derive reads, as the memo keys it (D6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MemoKey {
    /// The journey.
    pub journey: JourneyId,
    /// Its revision.
    pub revision: Revision,
    /// The deployment revision: the entities and aliases it was derived over.
    pub deployment: Revision,
    /// Today in the deployment's zone.
    pub today: Date,
    /// The caller's entities.
    pub viewer: BTreeSet<EntityKey>,
}

/// One journey derived: what every projection reads.
#[derive(Debug)]
pub(crate) struct Derivation {
    /// What it was derived from.
    pub key: MemoKey,
    /// Its graph, validated.
    pub graph: Graph,
    /// Its derive.
    pub derived: Derived,
}

impl Derivation {
    /// D3: derives `journey` over `deployment` with `inputs`.
    ///
    /// # Panics
    ///
    /// When the stored journey breaks an invariant, which every commit checked.
    pub fn new(
        journey: Journey,
        deployment: &Deployment,
        inputs: &cairn_schema::DeriveInputs,
    ) -> Self {
        let key = MemoKey {
            journey: journey.header.id,
            revision: journey.revision,
            deployment: deployment.revision,
            today: inputs.today,
            viewer: inputs.viewer.clone(),
        };
        let graph = match Graph::new(journey.graph, deployment) {
            Ok(graph) => graph,
            Err(violations) => panic!("a stored journey is valid: {violations:?}"),
        };
        let derived = crate::observe::timed_derive(|| {
            derive(&graph, Some(journey.header.created_on), inputs)
        });
        assert_eq!(derived.today(), key.today);
        Self {
            key,
            graph,
            derived,
        }
    }
}

/// The memo: recently read derivations, most recent last.
#[derive(Debug, Default)]
pub(crate) struct Memo {
    entries: Mutex<VecDeque<Arc<Derivation>>>,
}

impl Memo {
    /// The derivation for `key`, if the memo holds it; it becomes the most recent.
    pub fn get(&self, key: &MemoKey) -> Option<Arc<Derivation>> {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let position = entries.iter().position(|entry| entry.key == *key)?;
        let found = entries.remove(position)?;
        entries.push_back(Arc::clone(&found));
        Some(found)
    }

    /// Keeps `derivation` as the most recent, dropping the least recent past the bound.
    pub fn put(&self, derivation: Arc<Derivation>) {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        entries.retain(|entry| entry.key != derivation.key);
        entries.push_back(derivation);
        while entries.len() > MEMO_ENTRY_COUNT_MAX {
            entries.pop_front();
        }
        assert!(entries.len() <= MEMO_ENTRY_COUNT_MAX);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_schema::{DeriveInputs, RankConstants, from_yaml};

    fn journey(id: &str, revision: u32) -> Journey {
        let mut journey = Journey {
            header: from_yaml(&format!(
                "id: {id}\nname: One\nstatus: active\ncreated_at: \"2026-10-01T00:00:00Z\"\ncreated_on: \"2026-10-01\"\n"
            ))
            .unwrap(),
            revision: Revision::NONE.next(),
            graph: cairn_schema::Graph::default(),
        };
        for _ in 1..revision {
            journey.revision = journey.revision.next();
        }
        journey
    }

    fn derivation(id: &str, revision: u32, today: &str) -> Arc<Derivation> {
        let inputs = DeriveInputs {
            today: today.parse().unwrap(),
            timezone: "UTC".parse().unwrap(),
            rank: RankConstants::default(),
            viewer: BTreeSet::new(),
            deployment: Deployment::default(),
        };
        Arc::new(Derivation::new(
            journey(id, revision),
            &Deployment::default(),
            &inputs,
        ))
    }

    #[test]
    fn a_key_answers_only_its_own_derivation_and_the_least_recent_goes_first() {
        let memo = Memo::default();
        let first = derivation("j_0", 1, "2026-10-06");
        memo.put(Arc::clone(&first));
        let mut moved = first.key.clone();
        moved.revision = moved.revision.next();
        let mut tomorrow = first.key.clone();
        tomorrow.today = "2026-10-07".parse().unwrap();
        let mut viewer = first.key.clone();
        viewer.viewer.insert("e_someone".parse().unwrap());
        for other in [&moved, &tomorrow, &viewer] {
            assert!(memo.get(other).is_none(), "{other:?}");
        }
        assert!(Arc::ptr_eq(&memo.get(&first.key).unwrap(), &first));
        // Fill past the bound: reading the first keeps it, the next oldest goes.
        let others: Vec<_> = (1..=MEMO_ENTRY_COUNT_MAX)
            .map(|index| derivation(&format!("j_{index}"), 1, "2026-10-06"))
            .collect();
        for (index, other) in others.iter().enumerate() {
            memo.put(Arc::clone(other));
            if index == 0 {
                assert!(memo.get(&first.key).is_some());
            }
        }
        assert!(memo.get(&first.key).is_some());
        assert!(memo.get(&others[0].key).is_none(), "the least recent went");
        assert!(memo.get(&others[MEMO_ENTRY_COUNT_MAX - 1].key).is_some());
    }
}
