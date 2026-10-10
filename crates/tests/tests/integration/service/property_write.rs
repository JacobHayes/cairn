//! Property test for the service's write path over the memory store (the
//! engine's `Records` and the store stay separate,
//! decisions/2026-10-06-apply-works-over-loaded-records-and-replay-shares-its-write.md):
//! over generated operation sequences, each patch the service commits loads back exactly
//! as `apply` wrote it with `Records::write`, and each patch `apply` rejects is rejected
//! the same way and commits nothing.
#![cfg(test)]

mod property {
    use cairn_engine::testing::{arb_ops, fixed_inputs, journey_id};
    use cairn_engine::{Records, apply};
    use cairn_schema::{Mutations, Patch, PatchTarget, from_yaml};
    use cairn_service::{Call, WriteError, Written};
    use patina_dst_proptest::prelude::*;

    use crate::service::support::{domain, memory, run};

    proptest! {
        #[test]
        fn a_committed_patch_loads_back_as_apply_wrote_it(ops in prop::collection::vec(arb_ops(), 0..48)) {
            let (service, _) = memory();
            let inputs = fixed_inputs();
            let call = Call { actor: inputs.actor.clone(), now: inputs.at };
            let id = journey_id();
            let create = Patch {
                id: "p_create".parse().unwrap(),
                target: PatchTarget::Journey(id.clone()),
                base_revision: cairn_schema::Revision::NONE,
                deployment_revision: None,
                mutations: Mutations::new(vec![from_yaml("op: create_journey\nname: Generated\n").unwrap()]).unwrap(),
            };
            let mut records = apply(&Records::default(), &create, &inputs).unwrap().records().clone();
            let created = matches!(run(service.patch(&call, &domain(create))), Ok(Written::Applied { .. }));
            prop_assert!(created, "the journey is created");
            for (ordinal, op) in ops.into_iter().enumerate() {
                let mut patch = op.resolve(&records);
                patch.id = format!("p_generated_{ordinal}").parse().unwrap();
                let expected = apply(&records, &patch, &inputs);
                let written = run(service.patch(&call, &domain(patch)));
                match (expected, written) {
                    (Ok(applied), Ok(Written::Applied { receipt, .. })) => {
                        prop_assert_eq!(&receipt, &applied.change_set().receipt);
                        records = applied.records().clone();
                    }
                    (Err(rejection), Err(WriteError::Rejected(answered))) => {
                        prop_assert_eq!(rejection, answered);
                    }
                    (expected, written) => prop_assert!(false, "apply gave {expected:?}, the service {written:?}"),
                }
                let loaded = run(service.journey(&id)).unwrap();
                prop_assert_eq!(loaded.as_ref(), records.journeys.get(&id));
                prop_assert_eq!(&run(service.deployment()).unwrap(), &records.deployment);
            }
        }
    }
}
