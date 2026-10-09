//! Snoozing a container (PRD B6, D2, D3, D5, C9): accepted on a group and on a deliverable
//! with children, rejected for a target in the subtree or depending on it, held over every
//! descendant with the container named, cleared only by a transition on the container itself,
//! and refused on a descendant held only through it. Hand-built journeys, derived on
//! 2026-10-06 unless a test says otherwise.
#![cfg(test)]

use crate::support;

use cairn_engine::{Derived, DerivedJourney, Graph, Records};
use cairn_schema::{
    ListFlag, ListQuery, NodeKey, Rejection, SnoozeTarget, StallCause, Subject, Violation,
    ViolationCode,
};
use std::collections::BTreeSet;
use support::{add_nodes as add, key};

const BRANCH: &str = "{key: n_branch, id: branch, kind: group, title: Branch}";
const ONE: &str = "{key: n_one, id: one, parent: n_branch, kind: action, title: One}";
const TWO: &str =
    "{key: n_two, id: two, parent: n_branch, kind: action, title: Two, requires: [n_one]}";
const BOX: &str = "{key: n_box, id: box, kind: deliverable, title: Box}";
const PART: &str = "{key: n_part, id: part, parent: n_box, kind: action, title: Part}";
const OUTSIDE: &str = "{key: n_outside, id: outside, kind: action, title: Outside}";
const AFTER: &str = "{key: n_after, id: after, kind: action, title: After, requires: [n_branch]}";
const MEETING: &str = "{key: n_meeting, id: meeting, kind: milestone, title: Meeting}";

fn snooze(node: &str, until: &str) -> String {
    format!("- op: snooze\n  node: {node}\n  until: {until}\n")
}

fn transition(node: &str, transition: &str) -> String {
    format!("- op: transition\n  node: {node}\n  transition: {transition}\n")
}

fn unsnooze(node: &str) -> String {
    format!("- op: unsnooze\n  node: {node}\n")
}

fn journey() -> Records {
    support::journey(&add(&[
        BRANCH, ONE, TWO, BOX, PART, OUTSIDE, AFTER, MEETING,
    ]))
}

fn derive(records: &Records) -> (Graph, Derived) {
    let graph = support::journey_graph(records, support::JOURNEY);
    let derived = support::derived(records, support::JOURNEY);
    (graph, derived)
}

fn acting(derived: &Derived) -> Vec<&str> {
    derived
        .blocking()
        .acting_frontier()
        .iter()
        .map(NodeKey::as_str)
        .collect()
}

fn violations(records: &Records, mutations: &str) -> Vec<Violation> {
    match support::journey_patch(records, mutations) {
        Err(Rejection::Invalid { violations }) => violations.as_slice().to_vec(),
        other => panic!("expected a rejection: {other:#?}"),
    }
}

/// B6, D3: a group is snoozed, which holds every open descendant, naming the group, while
/// work outside it stays on the acting frontier; the group's dependents wait as before.
#[test]
fn a_group_snooze_holds_over_its_subtree() {
    let records = support::accepted(&journey(), &snooze("n_branch", "{date: \"2026-10-09\"}"));
    let (graph, derived) = derive(&records);
    let blocking = derived.blocking();
    assert_eq!(
        blocking
            .frontier()
            .iter()
            .map(NodeKey::as_str)
            .collect::<Vec<_>>(),
        ["n_meeting", "n_one", "n_outside", "n_part"],
        "D2: a snooze does not change actionability"
    );
    assert_eq!(acting(&derived), ["n_meeting", "n_outside", "n_part"]);
    let target = SnoozeTarget::Date("2026-10-09".parse().unwrap());
    for held in ["n_one", "n_two"] {
        let node = derived.node_derived(&graph, &key(held));
        assert_eq!(node.snoozed, Some(target.clone()), "{held}");
        assert_eq!(node.snoozed_via, Some(key("n_branch")), "{held}");
    }
    let group = derived.node_derived(&graph, &key("n_branch"));
    assert_eq!(group.snoozed, Some(target));
    assert_eq!(
        group.snoozed_via, None,
        "the container holds its own snooze"
    );
    let outside = derived.node_derived(&graph, &key("n_outside"));
    assert_eq!((outside.snoozed, outside.snoozed_via), (None, None));
    assert!(!blocking.deps_done(&key("n_after")), "B6: still blocks");
    // It lifts on its date.
    let lifted = support::derived_on(&records, support::JOURNEY, "2026-10-09".parse().unwrap());
    assert_eq!(
        lifted.blocking().snoozed_via(&key("n_one")),
        None,
        "derived from current state"
    );
}

/// B6: a deliverable with children is a container too, and is snoozed.
#[test]
fn a_deliverable_with_children_is_snoozed() {
    let records = support::accepted(&journey(), &snooze("n_box", "{node: n_meeting}"));
    let (_, derived) = derive(&records);
    assert_eq!(acting(&derived), ["n_meeting", "n_one", "n_outside"]);
    assert_eq!(
        derived.blocking().snoozed_via(&key("n_part")),
        Some(&key("n_box"))
    );
}

/// B6: a group with nothing open beneath it is not snoozed; a closed or out-of-scope one is not
/// either.
#[test]
fn a_group_with_no_work_left_is_not_snoozed() {
    let finished = support::accepted(
        &journey(),
        &format!(
            "{}{}",
            transition("n_one", "complete"),
            transition("n_two", "complete")
        ),
    );
    let found = violations(&finished, &snooze("n_branch", "{date: \"2026-10-09\"}"));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].code, ViolationCode::SnoozeNotActionable);
    let empty = support::journey(&add(&[BRANCH]));
    assert_eq!(
        violations(&empty, &snooze("n_branch", "{date: \"2026-10-09\"}"))[0].code,
        ViolationCode::SnoozeNotActionable
    );
}

/// B6: a target inside the subtree, an ancestor, or one that depends on anything in the
/// subtree is rejected with the container's path; a target that depends on nothing in it is
/// accepted.
#[test]
fn a_container_snooze_rejects_its_subtree_and_what_depends_on_it() {
    let records = journey();
    for (node, target) in [
        ("n_branch", "n_one"),
        ("n_branch", "n_two"),
        ("n_branch", "n_after"),
        ("n_box", "n_part"),
    ] {
        let found = violations(&records, &snooze(node, &format!("{{node: {target}}}")));
        assert!(
            found
                .iter()
                .all(|found| found.code == ViolationCode::SnoozeCycle),
            "{node} until {target}: {found:?}"
        );
        assert_eq!(found[0].related, [Subject::Node(key(target))]);
        assert!(
            found[0].at.subject == Some(Subject::Node(key(node))) && found[0].at.path.is_some(),
            "{node} until {target}: names the container"
        );
    }
    // A descendant's own snooze may not wait for its container's work either.
    assert_eq!(
        violations(&records, &snooze("n_part", "{node: n_box}"))[0].code,
        ViolationCode::SnoozeCycle
    );
    for (node, target) in [
        ("n_branch", "n_meeting"),
        ("n_branch", "n_outside"),
        ("n_box", "n_meeting"),
    ] {
        assert!(
            support::journey_patch(&records, &snooze(node, &format!("{{node: {target}}}"))).is_ok(),
            "{node} until {target}"
        );
    }
}

/// B6: a descendant completing does not clear its container's snooze; the container's own
/// transitions do, a deliverable's start included; a node target completing lifts it.
#[test]
fn only_a_transition_on_the_container_clears_it() {
    let snoozed = support::accepted(
        &journey(),
        &format!(
            "{}{}",
            snooze("n_branch", "{node: n_meeting}"),
            snooze("n_box", "{node: n_meeting}")
        ),
    );
    let done = support::accepted(&snoozed, &transition("n_one", "complete"));
    let stored = &support::graph(&done).state.snoozes;
    assert!(
        stored.contains_key(&key("n_branch")),
        "a descendant completing"
    );
    assert!(stored.contains_key(&key("n_box")));
    let (_, derived) = derive(&done);
    assert_eq!(
        derived.blocking().snoozed_via(&key("n_two")),
        Some(&key("n_branch"))
    );
    let part = support::accepted(&done, &transition("n_part", "start"));
    assert!(
        support::graph(&part)
            .state
            .snoozes
            .contains_key(&key("n_box"))
    );
    // The container's own transition clears: starting a container deliverable, skipping a group.
    let started = support::accepted(&part, &transition("n_box", "start"));
    assert!(
        !support::graph(&started)
            .state
            .snoozes
            .contains_key(&key("n_box"))
    );
    let skipped = support::accepted(
        &started,
        &transition("n_branch", "{skip: {reason: Not needed.}}"),
    );
    assert!(support::graph(&skipped).state.snoozes.is_empty());
    // The target completing lifts it from current state.
    let reached = support::accepted(&snoozed, &transition("n_meeting", "reach"));
    let (_, lifted) = derive(&reached);
    assert_eq!(acting(&lifted), ["n_one", "n_outside", "n_part"]);
    assert_eq!(lifted.blocking().snoozed_via(&key("n_one")), None);
}

/// B6: a descendant held only through its container cannot be unsnoozed on its own, naming the
/// container; its own snooze is independent and lifts alone; the container's unsnooze frees it.
#[test]
fn a_descendant_is_unsnoozed_through_its_container() {
    let snoozed = support::accepted(&journey(), &snooze("n_branch", "{node: n_meeting}"));
    let found = violations(&snoozed, &unsnooze("n_one"));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].code, ViolationCode::SnoozedThroughContainer);
    assert_eq!(found[0].related, [Subject::Node(key("n_branch"))]);
    assert!(found[0].message.contains("n_branch"));
    // An unrelated node is simply not snoozed.
    assert_eq!(
        violations(&snoozed, &unsnooze("n_outside"))[0].code,
        ViolationCode::UnresolvedReference
    );
    // Its own snooze is independent of the container's.
    let both = support::accepted(&snoozed, &snooze("n_one", "{date: \"2026-10-20\"}"));
    let (graph, derived) = derive(&both);
    let node = derived.node_derived(&graph, &key("n_one"));
    assert_eq!(
        node.snoozed,
        Some(SnoozeTarget::Date("2026-10-20".parse().unwrap()))
    );
    assert_eq!(node.snoozed_via, Some(key("n_branch")));
    let own_lifted = support::accepted(&both, &unsnooze("n_one"));
    let (graph, derived) = derive(&own_lifted);
    let node = derived.node_derived(&graph, &key("n_one"));
    assert_eq!(node.snoozed, Some(SnoozeTarget::Node(key("n_meeting"))));
    assert_eq!(node.snoozed_via, Some(key("n_branch")));
    // Unsnoozing the container frees what was held only through it, and the bulk order does
    // not matter: a descendant named after its container is just not snoozed.
    let freed = support::accepted(&both, &unsnooze("n_branch"));
    let (_, derived) = derive(&freed);
    assert_eq!(acting(&derived), ["n_meeting", "n_outside", "n_part"]);
    assert_eq!(
        derived.blocking().snoozed(&key("n_one")),
        Some(&SnoozeTarget::Date("2026-10-20".parse().unwrap())),
        "its own snooze stays after the container's lifts"
    );
    let bulk = format!("{}{}", unsnooze("n_branch"), unsnooze("n_two"));
    assert_eq!(
        violations(&snoozed, &bulk)[0].code,
        ViolationCode::UnresolvedReference
    );
}

/// D5: a journey whose acting frontier one container snooze empties is stalled, and the
/// diagnostic names the container's snooze once, with its target as a gate.
#[test]
fn stalled_names_the_container() {
    let records = support::accepted(
        &support::journey(&add(&[BRANCH, ONE, TWO, MEETING])),
        &format!(
            "{}{}",
            snooze("n_branch", "{node: n_meeting}"),
            snooze("n_meeting", "{date: \"2026-10-12\"}")
        ),
    );
    let (_, derived) = derive(&records);
    assert_eq!(acting(&derived), [] as [&str; 0]);
    let stalled = derived.blocking().stalled().expect("stalled (D5)");
    assert_eq!(
        stalled.waiting_on,
        [
            StallCause::Snooze {
                node: key("n_branch"),
                until: SnoozeTarget::Node(key("n_meeting")),
            },
            StallCause::Snooze {
                node: key("n_meeting"),
                until: SnoozeTarget::Date("2026-10-12".parse().unwrap()),
            },
        ]
    );
    assert!(!stalled.all_blocked);
}

/// D5: nested containers both holding: lifting the inner snooze alone leaves the journey
/// stalled on the outer one, so the diagnostic names both.
#[test]
fn stalled_names_every_container_above() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_outer, id: outer, kind: group, title: Outer}",
            "{key: n_inner, id: inner, parent: n_outer, kind: group, title: Inner}",
            "{key: n_leaf, id: leaf, parent: n_inner, kind: action, title: Leaf}",
        ])),
        &format!(
            "{}{}",
            snooze("n_inner", "{date: \"2026-10-08\"}"),
            snooze("n_outer", "{date: \"2026-10-12\"}")
        ),
    );
    let (_, derived) = derive(&records);
    let stalled = derived.blocking().stalled().expect("stalled (D5)");
    let named: Vec<&str> = stalled
        .waiting_on
        .iter()
        .map(|cause| match cause {
            StallCause::Snooze { node, .. } => node.as_str(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(named, ["n_inner", "n_outer"]);
    let later = support::derived_on(&records, support::JOURNEY, "2026-10-08".parse().unwrap());
    assert_eq!(
        later.blocking().snoozed_via(&key("n_leaf")),
        Some(&key("n_outer"))
    );
}

/// C9: the snoozed filter lists descendants snoozed through a container, with the container
/// named on their rows.
#[test]
fn the_snoozed_filter_lists_descendants() {
    let records = support::accepted(&journey(), &snooze("n_branch", "{node: n_meeting}"));
    let (graph, derived) = derive(&records);
    let journey = DerivedJourney::new(&graph, &derived);
    let query = ListQuery {
        flags: BTreeSet::from([ListFlag::Snoozed]),
        ..ListQuery::default()
    };
    let page = journey.list(&query, &BTreeSet::new()).unwrap();
    let mut rows: Vec<(&str, Option<&str>)> = page
        .rows
        .iter()
        .map(|row| {
            (
                row.key.as_str(),
                row.snoozed_via.as_ref().map(NodeKey::as_str),
            )
        })
        .collect();
    rows.sort_unstable();
    assert_eq!(
        rows,
        [
            ("n_branch", None),
            ("n_one", Some("n_branch")),
            ("n_two", Some("n_branch")),
        ]
    );
}
