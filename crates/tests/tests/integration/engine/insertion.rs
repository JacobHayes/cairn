//! Segment insertion (A21, B13, H5): the security review segment inserted twice into the
//! finished hiring journey.
#![cfg(test)]

use crate::engine::support;

use cairn_engine::Records;
use cairn_schema::{NodeKey, ParticipationSource, Provenance, Rejection, Revision, ViolationCode};
use support::key;

const HIRING: &str = "{journey: j_hiring}";

/// The first insertion's wiring: the review follows the panel being chosen.
const WIRE: &str = "  edges: [{node: {segment: n_review}, requires: {host: n_choose_panel}}]\n";

/// An `insert_segment` of the security review's first version, with the rest of its fields.
fn insert_review(insertion: &str, rest: &str) -> String {
    let segment = "{route: security-review, version: 1}";
    format!("- op: insert_segment\n  insertion: {insertion}\n  segment: {segment}\n{rest}")
}

fn hiring(records: &Records) -> &cairn_schema::Graph {
    &records.journeys[&"j_hiring".parse().unwrap()].graph
}

/// The journey's nodes with this title.
fn titled(records: &Records, title: &str) -> Vec<NodeKey> {
    let nodes = hiring(records).nodes.values();
    let found = nodes.filter(|node| node.title.as_str() == title);
    found.map(|node| node.key.clone()).collect()
}

/// The codes the whole insert is rejected with.
fn rejected(records: &Records, mutations: &str) -> Vec<ViolationCode> {
    let patch = support::patch_to(records, HIRING, mutations);
    match support::apply(records, &patch, &support::fixed_inputs()) {
        Err(Rejection::Invalid { violations }) => {
            violations.as_slice().iter().map(|v| v.code).collect()
        }
        other => panic!("rejected as invalid: {other:#?}"),
    }
}

/// The finished hiring journey with the first review inserted.
fn with_first_review() -> Records {
    let finished = support::with_segment(support::finished("hiring-loop"), "security-review");
    support::accepted_to(&finished, HIRING, &insert_review("i_one", WIRE))
}

/// How many nodes the inserted version of the security review holds.
fn segment_size(records: &Records) -> usize {
    let versions = records.versions.values();
    let found = versions
        .into_iter()
        .find(|v| v.route.as_str() == "security-review" && v.version.get() == 1);
    found.unwrap().graph.nodes.len()
}

/// The key of the first insertion's `reviewer` role.
fn reviewer_in(records: &Records) -> cairn_schema::RoleKey {
    let roles = hiring(records).roles.values();
    let found = roles
        .into_iter()
        .find(|role| role.id.as_str() == "reviewer");
    found.unwrap().key.clone()
}

/// A21, B13: a second default mapping fills `reviewer` twice, a cardinality mismatch and a
/// cycle reject the whole insert.
#[test]
fn an_insert_that_breaks_a_rule_is_rejected_whole() {
    let one = with_first_review();
    let reviewer = reviewer_in(&one);
    let panel = "  roles: {r_reviewer: {existing: r_panel}}\n";
    let cycle = "  omit: [n_who_reviews]\n  edges: [{node: {segment: n_review}, requires: {host: n_loop}}, {node: {host: n_loop}, requires: {segment: n_review}}]\n";
    for (rest, code) in [
        ("", ViolationCode::SeveralFillingDecisions),
        (panel, ViolationCode::InsertionInvalid),
        (cycle, ViolationCode::DependencyCycle),
    ] {
        assert!(rejected(&one, &insert_review("i_two", rest)).contains(&code));
    }
    let flip = format!("- op: edit_role\n  role: {{key: {reviewer}, id: reviewer, multi: true}}\n");
    assert!(rejected(&one, &flip).contains(&ViolationCode::InsertionInvalid));
}

/// A21, B13: with the second *Who reviews?* left out the insert is accepted; it gets other
/// keys and the next root id, and its owner resolves through the first insertion's role.
#[test]
fn a_segment_inserted_twice_gets_its_own_keys_and_resolves_through_the_first_role() {
    let one = with_first_review();
    let reviewer = reviewer_in(&one);

    let offer = "  omit: [n_who_reviews]\n  edges: [{node: {host: n_make_offer}, requires: {segment: n_review}}]\n";
    let two = support::accepted_to(&one, HIRING, &insert_review("i_two", offer));
    let graph = hiring(&two);
    let mut ids: Vec<_> = titled(&two, "Security review")
        .iter()
        .map(|key| graph.nodes.get(key).unwrap().id.as_str())
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, ["security-review", "security-review-2"]);
    let from_segment = |records: &Records| {
        let states = hiring(records).state.nodes.values();
        states
            .filter(|s| s.provenance == Provenance::FromSegment)
            .count()
    };
    let segment = segment_size(&one);
    assert_eq!(
        from_segment(&one),
        segment,
        "the first insertion copies every segment node, each with its initial state"
    );
    let added_by_two = from_segment(&two) - from_segment(&one);
    assert_eq!(
        added_by_two,
        segment - 1,
        "the second leaves its Who reviews? out"
    );
    assert_eq!(
        added_by_two,
        graph
            .insertions
            .values()
            .find(|i| i.key.as_str() == "i_two")
            .unwrap()
            .nodes
            .len(),
        "every node of the second insertion has its initial state"
    );
    for threat in titled(&two, "Threat model") {
        let owner = graph
            .nodes
            .get(&threat)
            .unwrap()
            .participations
            .as_map()
            .values()
            .next();
        assert!(matches!(owner, Some(ParticipationSource::Role(role)) if *role == reviewer));
    }
    assert!(graph.state.local_edits.contains_key(&key("n_make_offer")));
    let derived = support::derived(&two, "j_hiring");
    let mut frontier: Vec<_> = derived
        .blocking()
        .frontier()
        .iter()
        .map(|key| graph.nodes.get(key).unwrap().id.as_str())
        .collect();
    frontier.sort_unstable();
    assert_eq!(
        frontier,
        ["offer", "sensitivity", "sensitivity", "who-reviews"]
    );
}

/// H5: an insert against a revision the journey has moved past is stale; B13: removing an
/// insertion's last member removes the insertion.
#[test]
fn an_insert_at_an_old_revision_is_stale_and_the_last_member_takes_its_insertion() {
    let one = with_first_review();
    let mut stale = support::patch_to(&one, HIRING, &insert_review("i_three", ""));
    stale.base_revision = Revision::NONE;
    assert!(matches!(
        support::apply(&one, &stale, &support::fixed_inputs()),
        Err(Rejection::Stale { .. })
    ));
    let root_only = "  omit: [n_sensitivity, n_threat_model, n_who_reviews]\n";
    let two = support::accepted_to(&one, HIRING, &insert_review("i_two", root_only));
    let second = hiring(&two)
        .insertions
        .values()
        .find(|i| i.key.as_str() == "i_two");
    let removal = format!(
        "- op: remove_node\n  removal: {{node: {}}}\n",
        second.unwrap().nodes.keys().next().unwrap()
    );
    let removed = support::accepted_to(&two, HIRING, &removal);
    assert_eq!(
        hiring(&removed).insertions.len(),
        hiring(&one).insertions.len()
    );
}
