//! Property tests for conditions, attachments, and nodes in both reference forms:
//! every generated value survives JSON unchanged and writes the same bytes twice.

#[cfg(test)]
mod property {
    use cairn_schema::testing::*;
    use cairn_schema::{FileRefs, KeyRefs, NodeFieldValue};
    use patina_dst_proptest::prelude::*;
    use serde::Serialize;
    use serde::de::DeserializeOwned;

    fn json_round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(
        value: &T,
    ) -> Result<(), TestCaseError> {
        let text = serde_json::to_string(value).unwrap();
        let back: T = serde_json::from_str(&text)
            .map_err(|error| TestCaseError::fail(format!("{error}\n{text}")))?;
        prop_assert_eq!(&back, value);
        prop_assert_eq!(serde_json::to_string(&back).unwrap(), text);
        Ok(())
    }

    proptest! {
        #[test]
        fn conditions_round_trip(file in arb_condition::<FileRefs>(), resolved in arb_condition::<KeyRefs>()) {
            json_round_trip(&file)?;
            json_round_trip(&resolved)?;
            // Every referenced decision is found by the static walk (A5).
            prop_assert!(!file.decisions().is_empty());
        }

        #[test]
        fn attachments_round_trip(
            file in arb_resource::<FileRefs>(),
            resolved in arb_resource::<KeyRefs>(),
            annotation in arb_annotation(),
        ) {
            json_round_trip(&file)?;
            json_round_trip(&resolved)?;
            json_round_trip(&annotation)?;
        }

        #[test]
        fn nodes_round_trip(file in arb_node::<FileRefs>(), resolved in arb_node::<KeyRefs>()) {
            json_round_trip(&file)?;
            json_round_trip(&resolved)?;
        }

        #[test]
        fn a_written_field_reads_back(node in arb_node::<KeyRefs>(), value in arb_node_field_value::<KeyRefs>()) {
            json_round_trip(&value)?;
            let mut changed = node.clone();
            if value.clone().write(&mut changed) {
                prop_assert_eq!(NodeFieldValue::read(value.field(), &changed), Some(value));
            } else {
                prop_assert_eq!(changed, node);
            }
        }
    }
}
