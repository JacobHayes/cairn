//! Property tests for the documents: every generated document survives YAML and
//! JSON unchanged, and writing it twice gives the same bytes (ARCHITECTURE, File format).

#[cfg(test)]
mod property {
    use cairn_schema::testing::*;
    use cairn_schema::{from_json, from_yaml, to_json, to_yaml};
    use patina_dst_proptest::prelude::*;
    use serde::Serialize;
    use serde::de::DeserializeOwned;

    fn yaml_round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(
        value: &T,
    ) -> Result<(), TestCaseError> {
        let text = to_yaml(value).unwrap();
        let back: T =
            from_yaml(&text).map_err(|error| TestCaseError::fail(format!("{error}\n{text}")))?;
        prop_assert_eq!(&back, value);
        prop_assert_eq!(to_yaml(&back).unwrap(), text);
        Ok(())
    }

    fn json_round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(
        value: &T,
    ) -> Result<(), TestCaseError> {
        let text = to_json(value).unwrap();
        let back: T =
            from_json(&text).map_err(|error| TestCaseError::fail(format!("{error}\n{text}")))?;
        prop_assert_eq!(&back, value);
        prop_assert_eq!(to_json(&back).unwrap(), text);
        Ok(())
    }

    proptest! {
        #[test]
        fn route_files_round_trip(file in arb_route_file()) {
            yaml_round_trip(&file)?;
            json_round_trip(&file)?;
        }

        #[test]
        fn graphs_round_trip(graph in arb_graph()) {
            yaml_round_trip(&graph)?;
            json_round_trip(&graph)?;
        }

        #[test]
        fn domains_round_trip(
            journey in arb_journey(),
            route in arb_route(),
            version in arb_route_version(),
            deployment in arb_deployment(),
        ) {
            json_round_trip(&journey)?;
            json_round_trip(&route)?;
            json_round_trip(&version)?;
            json_round_trip(&deployment)?;
        }
    }
}
