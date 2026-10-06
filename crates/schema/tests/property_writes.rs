//! Property tests (rung 3) for patches, events, change sets, proposals, and touched sets.

#[cfg(test)]
mod property {
    use cairn_schema::testing::*;
    use cairn_schema::{ChangeClass, Domain, from_json, from_yaml, to_json, to_yaml};
    use patina_dst_proptest::prelude::*;
    use serde::Serialize;
    use serde::de::DeserializeOwned;

    fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(
        value: &T,
    ) -> Result<(), TestCaseError> {
        let json = to_json(value).unwrap();
        let back: T =
            from_json(&json).map_err(|error| TestCaseError::fail(format!("{error}\n{json}")))?;
        prop_assert_eq!(&back, value);
        let yaml = to_yaml(value).unwrap();
        let back: T =
            from_yaml(&yaml).map_err(|error| TestCaseError::fail(format!("{error}\n{yaml}")))?;
        prop_assert_eq!(&back, value);
        prop_assert_eq!(to_yaml(&back).unwrap(), yaml);
        Ok(())
    }

    /// The key itself, its domain, its graph, or the node it hangs off.
    fn widened(key: cairn_schema::RecordKey, how: u8) -> cairn_schema::RecordKey {
        use cairn_schema::{GraphKey, RecordKey};
        match (how, &key) {
            (1, _) => RecordKey::Domain(key.domain()),
            (2, RecordKey::InGraph { graph, .. }) => RecordKey::Graph(graph.clone()),
            (3, RecordKey::InGraph { graph, key: inner }) => match inner.node_scope() {
                Some(node) => RecordKey::InGraph {
                    graph: graph.clone(),
                    key: GraphKey::Node(node.clone()),
                },
                None => key.clone(),
            },
            _ => key,
        }
    }

    proptest! {
        #[test]
        fn patches_round_trip(patch in arb_patch()) {
            round_trip(&patch)?;
            prop_assert_eq!(from_json::<cairn_schema::Patch>(&to_json(&patch).unwrap()).unwrap().content_hash(), patch.content_hash());
        }

        #[test]
        fn change_sets_round_trip(change_set in arb_change_set()) {
            round_trip(&change_set)?;
        }

        #[test]
        fn scenarios_round_trip(scenario in arb_scenario()) {
            round_trip(&scenario)?;
        }

        #[test]
        fn proposals_round_trip(proposal in arb_proposal()) {
            round_trip(&proposal)?;
        }

        #[test]
        fn a_patch_touches_something_and_overlaps_itself(patch in arb_patch()) {
            let touched = patch.touched();
            prop_assert!(!touched.is_empty());
            prop_assert!(touched.overlaps(&touched));
        }

        #[test]
        fn overlap_is_symmetric(first in arb_patch(), second in arb_patch()) {
            prop_assert_eq!(first.touched().overlaps(&second.touched()), second.touched().overlaps(&first.touched()));
        }

        #[test]
        fn indexed_overlap_matches_the_pairwise_definition(
            first in prop::collection::vec(arb_write(), 1..6),
            second in prop::collection::vec(arb_write(), 0..4),
            widen in prop::collection::vec((any::<prop::sample::Index>(), 0u8..4), 0..4),
        ) {
            let keys = |writes: &[cairn_schema::Write]| writes.iter().flat_map(cairn_schema::Write::keys).collect::<Vec<_>>();
            let first = keys(&first);
            // Mix in addresses that cover some of the first set's, so overlaps happen often.
            let mut second = keys(&second);
            for (index, how) in widen {
                let key = index.get(&first).clone();
                second.push(widened(key, how));
            }
            let pairwise = first.iter().any(|a| second.iter().any(|b| a.overlaps(b)));
            let indexed = first.iter().cloned().collect::<cairn_schema::TouchedSet>().overlaps(&second.iter().cloned().collect());
            prop_assert_eq!(indexed, pairwise);
        }

        #[test]
        fn a_write_overlaps_its_own_graph(write in arb_write()) {
            // Every address a write names belongs to a domain, and a whole-domain address
            // overlaps it: what lets a coarse touch stand in for an unknown one (H5).
            for key in write.keys() {
                let domain = key.domain();
                let whole = cairn_schema::RecordKey::Domain(domain);
                prop_assert!(whole.overlaps(&key));
            }
        }

        #[test]
        fn route_patches_are_structural(patch in arb_patch()) {
            if matches!(patch.target.domain(), Domain::Route(_)) {
                prop_assert_eq!(patch.change_class(), ChangeClass::Structural);
            }
        }
    }
}
