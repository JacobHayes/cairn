//! Route insertion (B13, H5): the security review route inserted twice into the finished
//! hiring journey.
#![cfg(test)]

use crate::engine::support;

use cairn_engine::Records;
use cairn_schema::{NodeKey, ParticipationSource, Rejection, ViolationCode};

const HIRING: &str = "{journey: j_hiring}";

/// The first insertion's wiring: the review follows the panel being chosen.
const WIRE: &str = "  edges: [{node: {route: n_review}, requires: {host: n_choose_panel}}]\n";

/// An `insert_route` of the security review's first version, with the rest of its fields.
fn insert_review(insertion: &str, rest: &str) -> String {
    let route = "{route: security-review, version: 1}";
    format!("- op: insert_route\n  insertion: {insertion}\n  route: {route}\n{rest}")
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
    let finished = support::with_route(support::finished("hiring-loop"), "security-review");
    support::accepted_to(&finished, HIRING, &insert_review("i_one", WIRE))
}

/// How many nodes the inserted version of the security review holds.
fn route_size(records: &Records) -> usize {
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

/// B13: a second default mapping fills `reviewer` twice, a cardinality mismatch and a cycle
/// reject the whole insert.
#[test]
fn an_insert_that_breaks_a_rule_is_rejected_whole() {
    let one = with_first_review();
    let panel = "  roles: {r_reviewer: {existing: r_panel}}\n";
    let cycle = "  omit: [n_who_reviews]\n  edges: [{node: {route: n_review}, requires: {host: n_loop}}, {node: {host: n_loop}, requires: {route: n_review}}]\n";
    for (rest, code) in [
        ("", ViolationCode::SeveralFillingDecisions),
        (panel, ViolationCode::InsertionInvalid),
        (cycle, ViolationCode::DependencyCycle),
    ] {
        assert!(rejected(&one, &insert_review("i_two", rest)).contains(&code));
    }
}

/// B13: with the second *Who reviews?* left out the insert is accepted; it gets other
/// keys and the next root id, and its owner resolves through the first insertion's role.
#[test]
fn a_route_inserted_twice_gets_its_own_keys_and_resolves_through_the_first_role() {
    let one = with_first_review();
    let reviewer = reviewer_in(&one);

    let offer = "  omit: [n_who_reviews]\n  edges: [{node: {host: n_make_offer}, requires: {route: n_review}}]\n";
    let two = support::accepted_to(&one, HIRING, &insert_review("i_two", offer));
    let graph = hiring(&two);
    let mut ids: Vec<_> = titled(&two, "Security review")
        .iter()
        .map(|key| graph.nodes.get(key).unwrap().id.as_str())
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, ["security-review", "security-review-2"]);
    let inserted = |records: &Records| {
        let graph = hiring(records);
        let from = graph
            .nodes
            .values()
            .filter(|node| node.from_route.is_some());
        from.map(|node| node.key.clone()).collect::<Vec<_>>()
    };
    assert_eq!(
        inserted(&two).len(),
        2,
        "each inserted root keeps a pointer to the version it came from, and nothing else does"
    );
    for root in inserted(&two) {
        let found = graph.nodes.get(&root).unwrap().from_route.as_ref().unwrap();
        assert_eq!(
            (found.route.as_str(), found.version.get()),
            ("security-review", 1)
        );
        assert!(graph.nodes.get(&root).unwrap().parent.is_none());
    }
    let added_by_two = hiring(&two).nodes.len() - hiring(&one).nodes.len();
    assert_eq!(
        added_by_two,
        route_size(&one) - 1,
        "the second leaves its Who reviews? out"
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

/// B13: a route with two roots is inserted whole, each root keeps the pointer, and leaving
/// one root out places only the other; a root id or title is for a single placed root.
#[test]
fn a_route_with_several_roots_places_each_one_kept() {
    let two = "format: 1\nroute: pair\nname: Pair\nnodes:\n\
- {key: n_one, id: one, kind: action, title: One}\n\
- {key: n_two, id: two, kind: action, title: Two}\n";
    let finished = support::with_route_text(support::finished("hiring-loop"), two);
    let route = "{route: pair, version: 1}";
    let insert =
        |rest: &str| format!("- op: insert_route\n  insertion: i_pair\n  route: {route}\n{rest}");

    let both = support::accepted_to(&finished, HIRING, &insert(""));
    let roots: Vec<_> = hiring(&both)
        .nodes
        .values()
        .filter(|node| node.from_route.is_some())
        .map(|node| node.id.as_str())
        .collect();
    assert_eq!(roots.len(), 2, "each root keeps the pointer: {roots:?}");

    let one = support::accepted_to(&finished, HIRING, &insert("  omit: [n_two]\n"));
    let placed: Vec<_> = hiring(&one)
        .nodes
        .values()
        .filter(|node| node.from_route.is_some())
        .map(|node| node.title.as_str())
        .collect();
    assert_eq!(placed, ["One"]);

    let named = rejected(&finished, &insert("  root_title: Renamed\n"));
    assert_eq!(named, [ViolationCode::InsertionInvalid]);
    let none = rejected(&finished, &insert("  omit: [n_one, n_two]\n"));
    assert_eq!(none, [ViolationCode::InsertionInvalid]);
}
