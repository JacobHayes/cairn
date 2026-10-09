//! A1a, A16, D4, G4: `requires_note`, on the product launch with its hardening action marked.
//! Completion is refused until the action carries a note of its own (a link, and a note on the
//! journey, do not count), accepted when the note rides in the same patch, goes stale when the
//! last note is removed and clears when one is added.
#![cfg(test)]

use crate::engine::support;

use std::collections::BTreeSet;

use cairn_engine::{Applied, Records, apply};
use cairn_schema::{
    Guard, GuardFailure, Lineage, Rejection, RouteFile, SequentialKeys, ViolationCode, from_yaml,
};
use support::key;

const LAUNCH: &str = "j_launch";
const HARDENING: &str = "n_hardening";
const NOTE: &str = "- op: add_annotation\n  annotation: {key: a_note, node: n_hardening, note: \"Hardening covered the upgrade path.\"}\n";
const DONE: &str = "- op: transition\n  node: n_hardening\n  transition: complete\n";

/// The product launch's route file with the hardening action marked `requires_note`.
fn marked() -> RouteFile {
    let path = support::fixtures_root().join("product-launch/route.yaml");
    let text = std::fs::read_to_string(path).unwrap();
    let target = "  title: Hardening pass\n";
    assert!(text.contains(target));
    from_yaml(&text.replace(target, &format!("{target}  requires_note: true\n"))).unwrap()
}

/// The marked route as a graph.
fn marked_graph() -> cairn_schema::Graph {
    cairn_engine::from_file(&marked(), &mut SequentialKeys::default())
        .unwrap_or_else(|violations| panic!("{violations:#?}"))
        .into_document()
}

/// The product launch's scenario run over the marked route, through step `steps`.
fn launch_after(steps: usize) -> Records {
    let mut records = support::seeded("product-launch");
    let lineage = Lineage {
        route: "product-launch".parse().unwrap(),
        version: cairn_schema::VersionNumber::FIRST,
    };
    records.versions.get_mut(&lineage).unwrap().graph = marked_graph();
    for step in support::scenario("product-launch")
        .steps
        .as_slice()
        .iter()
        .take(steps)
    {
        records = support::step(&records, step).records().clone();
    }
    records
}

fn patch(records: &Records, mutations: &str) -> Result<Applied, Rejection> {
    let patch = support::patch_to(records, &format!("{{journey: {LAUNCH}}}"), mutations);
    apply(records, &patch, &support::fixed_inputs())
}

fn accepted(records: &Records, mutations: &str) -> Records {
    support::accepted_on(records, LAUNCH, mutations)
}

/// The failures that make the node stale.
fn stale(records: &Records) -> BTreeSet<GuardFailure> {
    let graph = support::journey_graph(records, LAUNCH);
    support::derived(records, LAUNCH).stale(&graph, &key(HARDENING))
}

/// The one violation a refused completion carries.
fn refusal(records: &Records, mutations: &str) -> cairn_schema::Violation {
    match patch(records, mutations) {
        Err(Rejection::Invalid { violations }) => match violations.as_slice() {
            [only] => only.clone(),
            other => panic!("one violation: {other:#?}"),
        },
        other => panic!("expected a refusal: {other:#?}"),
    }
}

/// Features finished, so the hardening action's only open guard is its note.
fn ready() -> Records {
    accepted(
        &launch_after(3),
        "- op: transition\n  node: n_features\n  transition: complete\n",
    )
}

/// A1a, D4, G4: refused without a note of its own, accepted with one in the same patch, stale
/// when the last note goes and clear when one returns.
#[test]
fn completion_needs_a_note_of_its_own_and_loses_it_as_stale() {
    let records = ready();
    let found = refusal(&records, DONE);
    assert_eq!(found.code, ViolationCode::GuardFailed);
    assert_eq!(found.bypassable, Some(Guard::HasNote));
    assert_eq!(found.failures, BTreeSet::from([GuardFailure::MissingNote]));

    // A link on the node and a note on the journey are not a note on the node.
    let others = "- op: add_annotation\n  annotation: {key: a_link, node: n_hardening, reference: \"https://example.org/report\"}\n\
- op: add_annotation\n  annotation: {key: a_journey, note: \"On the whole journey.\"}\n";
    let found = refusal(&records, &format!("{others}{DONE}"));
    assert_eq!(found.failures, BTreeSet::from([GuardFailure::MissingNote]));

    let done = accepted(&records, &format!("{NOTE}{DONE}"));
    assert!(stale(&done).is_empty());

    let removed = accepted(&done, "- op: remove_annotation\n  annotation: a_note\n");
    assert_eq!(stale(&removed), BTreeSet::from([GuardFailure::MissingNote]));
    let restored = accepted(&removed, NOTE);
    assert!(stale(&restored).is_empty());
}
