//! A domain document in the browser host (ARCHITECTURE, Web UI: data flow, version skew):
//! read with its engine version checked first, derived once, and projected; a document from
//! another engine version is refused before it is read, by every call that reads one.
#![cfg(test)]

use cairn_engine::testing::derive_inputs;
use cairn_schema::{Deployment, Journey, Revision, from_yaml};
use cairn_wasm::{Derivation, HostError};

/// The domain document of an empty journey, as the server builds it.
fn document() -> String {
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

#[test]
fn a_document_from_another_engine_version_is_refused_before_it_is_read() {
    let text = document();
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value["engine_version"] = "9.0.0".into();
    // A newer engine's document may carry what this one cannot read; skew is reported first.
    value["journey"]["unknown_to_this_engine"] = true.into();
    let newer = value.to_string();
    let refusals = [
        Derivation::new(&newer).map(|_| String::new()),
        cairn_wasm::apply(&newer, "{}"),
        cairn_wasm::preview(&newer, "{}"),
    ];
    for refused in refusals {
        match thrown(&refused.unwrap_err()) {
            HostError::VersionSkew { document, engine } => {
                assert_eq!(document.as_str(), "9.0.0");
                assert_eq!(engine, cairn_engine::engine_version());
            }
            other => panic!("expected version skew, got {other:?}"),
        }
    }
    assert!(Derivation::new(&text).is_ok(), "this engine's own");
}
