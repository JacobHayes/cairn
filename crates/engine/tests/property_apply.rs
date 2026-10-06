//! Property tests (rung 3) for the write path (PRACTICES, Testing > Engine; J2, J3, A15, A17):
//! over generated journeys and operation sequences, apply then replay equals apply, a
//! rejected patch leaves the records unchanged and says why, every accepted patch emits one
//! event per mutation and advances the revision by one, every accepted graph holds every
//! invariant, and apply is a function of its inputs.

#[cfg(test)]
mod property {
    use cairn_engine::testing::{arb_graph, arb_ops, fixed_inputs, journey_id};
    use cairn_engine::{Graph, apply, replay};
    use cairn_schema::Rejection;
    use patina_dst_proptest::prelude::*;

    proptest! {
        #[test]
        fn apply_then_replay_matches(initial in arb_graph(), ops in prop::collection::vec(arb_ops(), 1..64)) {
            let inputs = fixed_inputs();
            let mut records = initial.clone();
            let mut events = Vec::new();
            for op in ops {
                let patch = op.resolve(&records);
                let before = records.clone();
                match apply(&records, &patch, &inputs) {
                    Ok(applied) => {
                        prop_assert_eq!(applied.events().len(), patch.mutations.len());
                        prop_assert_eq!(applied.revision(), patch.base_revision.next());
                        events.extend(applied.events().iter().cloned());
                        records = applied.records().clone();
                        let graph = records.journeys[&journey_id()].graph.clone();
                        prop_assert!(Graph::new(graph, &records.deployment).is_ok(), "an accepted graph holds every invariant");
                    }
                    Err(Rejection::Invalid { violations }) => {
                        prop_assert!(!violations.as_slice().is_empty());
                        prop_assert_eq!(&records, &before);
                    }
                    Err(other) => prop_assert!(false, "a generated patch is never stale: {other:?}"),
                }
            }
            prop_assert_eq!(replay(&initial, &events), records);
        }

        #[test]
        fn apply_is_a_function_of_its_inputs(initial in arb_graph(), op in arb_ops()) {
            let patch = op.resolve(&initial);
            let first = apply(&initial, &patch, &fixed_inputs());
            let second = apply(&initial, &patch, &fixed_inputs());
            prop_assert_eq!(&first, &second);
            if let (Ok(first), Ok(second)) = (&first, &second) {
                let json = |applied: &cairn_engine::Applied| cairn_schema::to_json(applied.change_set()).unwrap();
                prop_assert_eq!(json(first), json(second));
            }
        }
    }
}
