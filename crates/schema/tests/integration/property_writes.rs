//! Property tests for patches, events, change sets, proposals, and touched sets.

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
        fn route_patches_are_structural(patch in arb_patch()) {
            if matches!(patch.target.domain(), Domain::Route(_)) {
                prop_assert_eq!(patch.change_class(), ChangeClass::Structural);
            }
        }
    }
}
