//! Property tests for the write path (PRACTICES, Testing > Engine; J2, J3, A15, A17):
//! over generated journeys and sequences of generated patches, each of one to several
//! mutations applied together, apply then replay equals apply, a rejected patch leaves the
//! records unchanged and says why, every accepted patch emits one event per mutation and
//! advances the revision by one, every accepted graph holds every invariant, a structural
//! patch applies whole as it does one mutation at a time, and apply is a function of its
//! inputs.

#[cfg(test)]
mod property {
    use cairn_engine::testing::{
        arb_graph, arb_patch, arb_structure_patch, fixed_inputs, journey_id, resolve_in_sequence,
        resolve_patch,
    };
    use cairn_engine::{Graph, apply, replay};
    use cairn_schema::Rejection;
    use patina_dst_proptest::prelude::*;

    proptest! {
        #[test]
        fn apply_then_replay_matches(initial in arb_graph(), patches in prop::collection::vec(arb_patch(), 1..24)) {
            let inputs = fixed_inputs();
            let mut records = initial.clone();
            let mut events = Vec::new();
            for ops in patches {
                let patch = resolve_patch(&ops, &records);
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

        /// A18, A17: a structural patch whose mutations are each accepted applied one at a
        /// time is accepted whole and leaves the same graph: a removal sees what the
        /// mutations before it in its patch added, moved, or linked.
        #[test]
        fn a_structural_patch_applies_whole_as_one_mutation_at_a_time(initial in arb_graph(), ops in arb_structure_patch()) {
            let (patch, sequence) = resolve_in_sequence(&ops, &initial);
            if let Some(sequence) = sequence {
                let whole = apply(&initial, &patch, &fixed_inputs());
                prop_assert!(whole.is_ok(), "{whole:?}");
                if let Ok(whole) = whole {
                    let graph = |records: &cairn_engine::Records| records.journeys[&journey_id()].graph.clone();
                    prop_assert_eq!(graph(whole.records()), graph(&sequence));
                }
            }
        }

        #[test]
        fn apply_is_a_function_of_its_inputs(initial in arb_graph(), ops in arb_patch()) {
            let patch = resolve_patch(&ops, &initial);
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
