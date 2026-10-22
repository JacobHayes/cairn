//! Property tests for identifiers, bounded text, and numbers: every generated value
//! survives its text and JSON forms unchanged.

#[cfg(test)]
mod property {
    use cairn_schema::testing::*;
    use patina_dst_proptest::prelude::*;
    use serde::Serialize;
    use serde::de::DeserializeOwned;

    fn json_round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: &T) {
        let text = serde_json::to_string(value).unwrap();
        let back: T = serde_json::from_str(&text).unwrap();
        assert_eq!(&back, value);
    }

    proptest! {
        /// The parser and the published pattern accept the same text: nothing else makes one
        /// follow the other. The strategy mixes slugs, up to past the id byte limit, with
        /// text that has uppercase, separators, whitespace, a newline, and non-ASCII.
        #[test]
        fn a_slug_parses_exactly_when_its_schema_matches(
            text in prop_oneof!["[a-z0-9][a-z0-9_-]{0,70}", "[a-zA-Z0-9_./ \né-]{0,70}"],
        ) {
            let schema = schemars::schema_for!(cairn_schema::Slug).to_value();
            let validator = jsonschema::draft202012::new(&schema).unwrap();
            let published = validator.is_valid(&serde_json::Value::String(text.clone()));
            prop_assert_eq!(text.parse::<cairn_schema::Slug>().is_ok(), published, "{:?}", text);
        }

        #[test]
        fn identifiers_round_trip(
            slug in arb_slug(),
            path in arb_path(),
            node in arb_node_key(),
            role in arb_role_key(),
            kind in arb_kind_key(),
            identity in arb_identity_key(),
            attachment in arb_attachment_key(),
            journey in arb_journey_id(),
            patch in arb_patch_id(),
            proposal in arb_proposal_id(),
            user in arb_user_id(),
            agent in arb_agent_id(),
            route in arb_route_id(),
        ) {
            json_round_trip(&slug);
            json_round_trip(&path);
            prop_assert_eq!(path.to_string().parse::<cairn_schema::Path>().unwrap(), path);
            json_round_trip(&node);
            json_round_trip(&role);
            json_round_trip(&kind);
            json_round_trip(&identity);
            json_round_trip(&attachment);
            json_round_trip(&journey);
            json_round_trip(&patch);
            json_round_trip(&proposal);
            json_round_trip(&user);
            json_round_trip(&agent);
            json_round_trip(&route);
        }

        #[test]
        fn text_and_numbers_round_trip(
            title in arb_title(),
            markdown in arb_markdown(),
            reason in arb_reason(),
            url in arb_url(),
            email in arb_email(),
            revision in arb_revision(),
            version in arb_version_number(),
            days in arb_days(),
            weight in arb_weight(),
            date in arb_date(),
            timestamp in arb_timestamp(),
        ) {
            json_round_trip(&title);
            json_round_trip(&markdown);
            json_round_trip(&reason);
            json_round_trip(&url);
            json_round_trip(&email);
            json_round_trip(&revision);
            json_round_trip(&version);
            json_round_trip(&days);
            json_round_trip(&weight);
            json_round_trip(&date);
            json_round_trip(&timestamp);
        }

        #[test]
        fn a_key_never_parses_as_another_key_type(node in arb_node_key()) {
            prop_assert!(node.as_str().parse::<cairn_schema::RoleKey>().is_err());
            prop_assert!(node.as_str().parse::<cairn_schema::IdentityKey>().is_err());
        }
    }
}
