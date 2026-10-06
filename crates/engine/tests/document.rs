//! The domain document (ARCHITECTURE, Terms: Domain document): stored state, the derive
//! inputs, and the engine version, with nothing derived in it, so deriving what arrives
//! gives what the host derives at the same inputs.
#![cfg(test)]

mod support;

use cairn_engine::project::document;
use cairn_engine::{Graph, derive};
use cairn_schema::{DomainDocument, Journey};

/// Every fixture's journey after its whole scenario, as the host would hold it.
fn finished_journeys() -> Vec<(String, Journey, cairn_schema::Deployment)> {
    let mut found = Vec::new();
    for name in support::fixture_names() {
        let records = support::finished(&name);
        for journey in records.journeys.values() {
            found.push((name.clone(), journey.clone(), records.deployment.clone()));
        }
    }
    assert!(!found.is_empty(), "the fixtures hold journeys");
    found
}

#[test]
fn a_document_sent_and_derived_gives_what_the_host_derives() {
    for (name, journey, deployment) in finished_journeys() {
        let mut inputs = cairn_engine::testing::derive_inputs(deployment);
        inputs.viewer = inputs
            .deployment
            .entities
            .values()
            .take(1)
            .map(|entity| entity.key.clone())
            .collect();
        let created_on = journey.header.created_on;
        let host_graph = Graph::new(journey.graph.clone(), &inputs.deployment).unwrap();
        let on_host = derive(&host_graph, Some(created_on), &inputs);

        let sent = serde_json::to_string(&document(&journey, &inputs)).unwrap();
        let received: DomainDocument = serde_json::from_str(&sent).unwrap();
        let graph = Graph::new(received.journey.graph, &received.inputs.deployment).unwrap();
        let in_browser = derive(
            &graph,
            Some(received.journey.header.created_on),
            &received.inputs,
        );
        assert_eq!(in_browser, on_host, "{name}");
    }
}

#[test]
fn a_document_holds_the_stored_journey_and_nothing_derived() {
    for (name, journey, deployment) in finished_journeys() {
        let inputs = cairn_engine::testing::derive_inputs(deployment);
        let sent = serde_json::to_value(document(&journey, &inputs)).unwrap();
        let fields: Vec<&str> = sent
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(fields, ["engine_version", "inputs", "journey"], "{name}");
        assert_eq!(
            sent["journey"],
            serde_json::to_value(&journey).unwrap(),
            "{name}"
        );
        assert_eq!(
            sent["inputs"],
            serde_json::to_value(&inputs).unwrap(),
            "{name}"
        );
        assert_eq!(sent["engine_version"], env!("CARGO_PKG_VERSION"), "{name}");
    }
}
