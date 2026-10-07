//! Property tests (rung 3) for rejections and the derived shapes: every generated value
//! survives the JSON wire form unchanged.

#[cfg(test)]
mod property {
    use cairn_schema::testing::*;
    use cairn_schema::{from_json, to_json};
    use patina_dst_proptest::prelude::*;
    use serde::Serialize;
    use serde::de::DeserializeOwned;

    fn json_round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(
        value: &T,
    ) -> Result<(), TestCaseError> {
        let json = to_json(value).unwrap();
        let back: T =
            from_json(&json).map_err(|error| TestCaseError::fail(format!("{error}\n{json}")))?;
        prop_assert_eq!(&back, value);
        prop_assert_eq!(to_json(&back).unwrap(), json);
        Ok(())
    }

    proptest! {
        #[test]
        fn rejections_round_trip(rejection in arb_rejection()) {
            json_round_trip(&rejection)?;
        }

        #[test]
        fn derived_values_round_trip(derived in arb_derived(), consequences in arb_consequences()) {
            json_round_trip(&derived)?;
            json_round_trip(&consequences)?;
        }

        #[test]
        fn domain_documents_round_trip(document in arb_domain_document()) {
            json_round_trip(&document)?;
        }
    }
}
