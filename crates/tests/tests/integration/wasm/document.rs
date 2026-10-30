//! A domain document in the browser (ARCHITECTURE, Web UI: data flow): read, derived
//! once, and projected.
#![cfg(test)]

use cairn_engine::testing::derive_inputs;
use cairn_schema::{Deployment, Journey, Revision, from_yaml};
use cairn_wasm::{Derivation, HostError};

/// The domain document of an empty journey, as the server builds it.
pub fn document() -> String {
    let journey = Journey {
        header: from_yaml(
            "id: j_empty\nname: Empty\nstatus: active\ncreated_at: \"2026-10-01T00:00:00Z\"\ncreated_on: \"2026-10-01\"\n",
        )
        .unwrap(),
        revision: Revision::NONE.next(),
        graph: cairn_schema::Graph::default(),
    };
    let document = cairn_engine::project::document(&journey, &derive_inputs(Deployment::default()));
    serde_json::to_string(&document).unwrap()
}

fn thrown(error: &str) -> HostError {
    serde_json::from_str(error).unwrap()
}

#[test]
fn a_document_derives_once_and_answers_each_projection_or_names_what_is_missing() {
    let derivation = Derivation::new(&document()).unwrap();
    let derived: cairn_schema::Derived = serde_json::from_str(&derivation.derived()).unwrap();
    assert_eq!(derived.today.to_string(), "2026-10-06");
    let summary = derivation
        .project(r#"{"projection":"status_summary"}"#)
        .unwrap();
    assert!(serde_json::from_str::<cairn_schema::StatusSummary>(&summary).is_ok());
    let missing = derivation.project(r#"{"projection":"trace","key":"n_absent"}"#);
    assert!(matches!(
        thrown(&missing.unwrap_err()),
        HostError::Missing { .. }
    ));
    let unreadable = derivation.project(r#"{"projection":"nothing"}"#);
    assert!(matches!(
        thrown(&unreadable.unwrap_err()),
        HostError::Unreadable { .. }
    ));
}

/// A patch applied in the browser reports what it caused as the server does, with both sides
/// derived at the minute of the apply: a pin to a time already past is overdue even though the
/// document was read a minute before it.
#[test]
fn a_local_apply_derives_both_sides_at_the_minute_of_the_apply() {
    use crate::engine::support;
    let records = support::journey(&support::add_nodes(&[
        "{key: n_work, id: work, kind: action, title: Work}",
    ]));
    let journey = records.journeys[&support::JOURNEY.parse().unwrap()].clone();
    let mut inputs = derive_inputs(Deployment::default());
    inputs.now = "2026-10-06T08:14:00Z".parse().unwrap();
    let document =
        serde_json::to_string(&cairn_engine::project::document(&journey, &inputs)).unwrap();
    let patch = support::patch_to(
        &records,
        &format!("{{journey: {}}}", support::JOURNEY),
        "- op: set_pin\n  node: n_work\n  date: \"2026-10-06T08:15+00:00\"\n",
    );
    let request = serde_json::json!({
        "patch": patch,
        "at": "2026-10-06T08:16:00Z",
        "actor": {"user": "u_local"},
    });
    let answer = cairn_wasm::apply(&document, &request.to_string()).unwrap();
    let applied: cairn_wasm::AppliedLocally = serde_json::from_str(&answer).unwrap();
    assert_eq!(applied.consequences.overdue.len(), 1);
}
