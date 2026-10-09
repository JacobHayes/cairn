//! C9's list (each filter on a fixture state, in-journey search, sorting, paging), C10's next
//! list (rank order, breadcrumbs and terms, re-sorting by one signal, "only mine", by kind, a
//! subtree, "prioritize for me", the stalled diagnostic), and E4's "mine". Derived at
//! 2026-10-06 unless a test says otherwise.
#![cfg(test)]

use crate::engine::support;

use std::collections::BTreeSet;

use cairn_engine::{Derived, DerivedJourney, Graph, Records};
use cairn_schema::{
    DisplayState, EntityKey, KindKey, ListFlag, ListQuery, NextQuery, NodeKind, SortBy, State,
};
use support::{add_nodes as add, key};

const VENDOR: &str = "j_vendor_eval";

/// A journey's graph and its derive.
struct Derive {
    graph: Graph,
    derived: Derived,
}

impl Derive {
    fn on(records: &Records, journey: &str, today: &str) -> Self {
        Self {
            graph: support::journey_graph(records, journey),
            derived: support::derived_on(records, journey, today.parse().unwrap()),
        }
    }

    fn vendor(steps: usize) -> Self {
        Self::on(&support::vendor_after(steps), VENDOR, "2026-10-06")
    }

    fn test(records: &Records) -> Self {
        Self::on(records, support::JOURNEY, "2026-10-06")
    }

    fn journey(&self) -> DerivedJourney<'_> {
        DerivedJourney::new(&self.graph, &self.derived)
    }

    /// The keys the list query matches, in its order, across every page.
    fn listed(&self, query: &ListQuery, viewer: &[&str]) -> Vec<String> {
        let viewer = entities(viewer);
        let mut keys = Vec::new();
        let mut query = query.clone();
        loop {
            let page = self.journey().list(&query, &viewer).unwrap();
            keys.extend(page.rows.iter().map(|row| row.key.as_str().to_owned()));
            match page.next {
                Some(cursor) => query.cursor = cursor,
                None => return keys,
            }
        }
    }

    fn next(&self, query: &NextQuery, viewer: &[&str]) -> Vec<String> {
        let next = self.journey().next(query, &entities(viewer)).unwrap();
        next.items
            .iter()
            .map(|row| row.key.as_str().to_owned())
            .collect()
    }
}

fn entities(keys: &[&str]) -> BTreeSet<EntityKey> {
    keys.iter().map(|key| key.parse().unwrap()).collect()
}

fn sorted(keys: Vec<String>) -> Vec<String> {
    let set: BTreeSet<String> = keys.into_iter().collect();
    set.into_iter().collect()
}

fn flag(flag: ListFlag) -> ListQuery {
    ListQuery {
        flags: BTreeSet::from([flag]),
        ..ListQuery::default()
    }
}

/// C9, E1: "unassigned", "by owner", and "mine" on the vendor evaluation before and after its
/// owner and stakeholders are decided. Before, every node that is not a group (a group is
/// never unassigned) is unassigned: all are in scope and unfinished at this step.
#[test]
fn the_people_filters_keep_whose_nodes_they_name() {
    let decided = Derive::vendor(2);
    let all = decided.graph.document().nodes.len();
    let leaves = decided
        .graph
        .document()
        .nodes
        .values()
        .filter(|node| node.kind() != NodeKind::Group)
        .count();
    assert!(leaves < all);
    assert_eq!(
        Derive::vendor(1)
            .listed(&flag(ListFlag::Unassigned), &[])
            .len(),
        leaves
    );
    assert_eq!(decided.listed(&flag(ListFlag::Unassigned), &[]).len(), 0);
    let lead = ListQuery {
        owner: Some("e_lead".parse().unwrap()),
        ..ListQuery::default()
    };
    assert_eq!(decided.listed(&lead, &[]).len(), all);
    assert_eq!(
        sorted(decided.listed(&flag(ListFlag::Mine), &["e_stakeholder_a"])),
        [
            "n_final_report",
            "n_final_review",
            "n_findings",
            "n_findings_reviewer",
            "n_reporting",
            "n_review_opens",
        ]
    );
}

/// C9: each state filter on a fixture state, as the set of nodes it keeps.
#[test]
fn each_state_filter_keeps_what_it_names() {
    let cases: Vec<(Derive, ListFlag, Vec<&str>)> = vec![
        (
            Derive::vendor(1),
            ListFlag::DecisionsNeeded,
            vec![
                "n_meeting_date",
                "n_partner_runs",
                "n_purpose",
                "n_who_informed",
                "n_who_owns",
            ],
        ),
        (
            Derive::vendor(1),
            ListFlag::NeedsBreakdown,
            vec!["n_workload"],
        ),
        (Derive::vendor(4), ListFlag::Active, vec!["n_access"]),
        (
            Derive::vendor(7),
            ListFlag::NextUp,
            vec![
                "n_decision_meeting",
                "n_workload_ingest",
                "n_workload_query",
            ],
        ),
        (Derive::vendor(7), ListFlag::Snoozed, vec!["n_baseline"]),
        (
            Derive::on(&support::vendor_after(7), VENDOR, "2026-11-07"),
            ListFlag::SnoozedAndOverdue,
            vec!["n_baseline"],
        ),
        (
            Derive::on(&support::vendor_after(3), VENDOR, "2026-11-01"),
            ListFlag::Overdue,
            vec!["n_access"],
        ),
        (stale_findings(), ListFlag::Stale, vec!["n_findings"]),
        (
            Derive::on(
                &support::finished("product-launch"),
                "j_launch",
                "2026-10-06",
            ),
            ListFlag::Shortfall,
            vec!["n_code_freeze", "n_launch"],
        ),
    ];
    for (derive, filter, expected) in cases {
        assert_eq!(
            sorted(derive.listed(&flag(filter), &[])),
            expected,
            "{filter:?}"
        );
    }
    let blocked = Derive::vendor(1).listed(&flag(ListFlag::Blocked), &[]);
    assert!(blocked.contains(&"n_access".to_owned()) && !blocked.contains(&"n_kickoff".to_owned()));
}

/// The vendor evaluation with a dependency inserted after its findings were done.
fn stale_findings() -> Derive {
    let records = support::accepted_on(
        &support::vendor_after(8),
        VENDOR,
        "- op: add_node\n  node: {key: n_extra, id: extra, parent: n_reporting, kind: action, title: Extra}\n- op: add_edge\n  edge: {node: n_findings, requires: n_extra}\n",
    );
    Derive::on(&records, VENDOR, "2026-10-06")
}

/// C9: filters by group, kind, and state combine; text search finds a node by its title, a
/// note on it, and a resource's link, ignoring case.
#[test]
fn list_filters_combine_and_search_reads_notes_and_resources() {
    let setup_actions = ListQuery {
        within: Some(key("n_setup")),
        kinds: BTreeSet::from([NodeKind::Action]),
        ..ListQuery::default()
    };
    assert_eq!(
        sorted(Derive::vendor(1).listed(&setup_actions, &[])),
        ["n_plan_draft", "n_plan_review"]
    );
    let done = ListQuery {
        states: BTreeSet::from([State::Done]),
        ..ListQuery::default()
    };
    assert_eq!(Derive::vendor(5).listed(&done, &[]), ["n_access"]);
    let started = Derive::vendor(4);
    for (text, expected) in [
        ("waiting on the ENVIRONMENT team", "n_access"),
        ("test PLAN", "n_plan"),
        ("templates/evaluation-report", "n_final_report"),
    ] {
        let search = ListQuery {
            text: Some(text.parse().unwrap()),
            ..ListQuery::default()
        };
        assert_eq!(started.listed(&search, &[]), [expected], "{text}");
    }
}

/// C9, D8: the list filters by the state each node shows, so the not-relevant nodes can be left
/// out (or asked for) as the plan's count does.
#[test]
fn the_list_filters_by_display_state() {
    let decided = Derive::vendor(2);
    let every = decided
        .journey()
        .list(&ListQuery::default(), &BTreeSet::new())
        .unwrap()
        .rows;
    let ruled_out = |keep: bool| -> Vec<String> {
        let rows = every.iter();
        rows.filter(|row| (row.display_state == DisplayState::NotRelevant) == keep)
            .map(|row| row.key.as_str().to_owned())
            .collect()
    };
    assert!(
        !ruled_out(true).is_empty(),
        "the answer ruled something out"
    );
    let only = |states: &[DisplayState]| ListQuery {
        display_states: states.iter().copied().collect(),
        ..ListQuery::default()
    };
    assert_eq!(
        sorted(decided.listed(&only(&[DisplayState::NotRelevant]), &[])),
        sorted(ruled_out(true))
    );
    let rest: Vec<DisplayState> = every
        .iter()
        .map(|row| row.display_state)
        .filter(|state| *state != DisplayState::NotRelevant)
        .collect();
    assert_eq!(
        sorted(decided.listed(&only(&rest), &[])),
        sorted(ruled_out(false))
    );
}

const PAGE: usize = 200;

/// A journey of `count` independent actions.
fn many(count: usize) -> Records {
    let nodes: Vec<String> = (0..count)
        .map(|at| format!("{{key: n_a{at:03}, id: a{at:03}, kind: action, title: Action}}"))
        .collect();
    let nodes: Vec<&str> = nodes.iter().map(String::as_str).collect();
    support::journey(&add(&nodes))
}

/// C9: the list pages `page_item_count_max` rows at a time with the total.
#[test]
fn the_list_pages_with_a_cursor() {
    let derive = Derive::test(&many(PAGE + 5));
    let first = derive
        .journey()
        .list(&ListQuery::default(), &BTreeSet::new())
        .unwrap();
    assert_eq!((first.rows.len(), first.total), (PAGE, 205));
    let rest = ListQuery {
        cursor: first.next.unwrap(),
        ..ListQuery::default()
    };
    let second = derive.journey().list(&rest, &BTreeSet::new()).unwrap();
    assert_eq!((second.rows.len(), second.next), (5, None));
}

/// C10: the next list is the acting frontier in rank order, each item with its breadcrumb and
/// rank terms; it filters to kinds, a subtree, and the viewer's items.
#[test]
fn next_is_the_ranked_acting_frontier_with_breadcrumbs() {
    let kicked_off = Derive::vendor(3);
    let next = kicked_off
        .journey()
        .next(&NextQuery::default(), &BTreeSet::new())
        .unwrap();
    let keys: Vec<&str> = next.items.iter().map(|row| row.key.as_str()).collect();
    assert_eq!(keys, ["n_access", "n_decision_meeting", "n_workload"]);
    assert_eq!(next.items[0].ancestors, [key("n_setup")]);
    let rank = next.items[0].rank.unwrap().rank.get();
    assert_eq!(
        Some(rank),
        kicked_off.derived.ranking().rank(&key("n_access"))
    );
    let within = NextQuery {
        within: Some(key("n_setup")),
        ..NextQuery::default()
    };
    assert_eq!(kicked_off.next(&within, &[]), ["n_access", "n_workload"]);
    let decisions = NextQuery {
        kinds: BTreeSet::from([NodeKind::Decision]),
        ..NextQuery::default()
    };
    assert_eq!(
        Derive::vendor(1).next(&decisions, &[]),
        [
            "n_partner_runs",
            "n_meeting_date",
            "n_purpose",
            "n_who_informed",
            "n_who_owns"
        ]
    );
    let mine = NextQuery {
        mine: true,
        ..NextQuery::default()
    };
    let decided = Derive::vendor(2);
    assert_eq!(
        decided.next(&mine, &["e_lead"]),
        ["n_kickoff", "n_decision_meeting"]
    );
    assert_eq!(decided.next(&mine, &["e_stakeholder_a"]).len(), 0);
}

/// C10: re-sorting by one signal: a heavy action with no deadline outranks nothing urgent, but
/// a milestone pinned to tomorrow ranks first; by gravity the heavy action leads, by slack the
/// milestone.
#[test]
fn next_re_sorts_by_one_signal() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_heavy, id: heavy, kind: action, title: Heavy, weight: 10}",
            "{key: n_soon, id: soon, kind: milestone, title: Soon}",
        ])),
        "- op: set_pin\n  node: n_soon\n  date: \"2026-10-07\"\n",
    );
    let derive = Derive::test(&records);
    let by = |sort| {
        derive.next(
            &NextQuery {
                sort,
                ..NextQuery::default()
            },
            &[],
        )
    };
    assert_eq!(by(SortBy::Rank), ["n_soon", "n_heavy"]);
    assert_eq!(by(SortBy::Gravity), ["n_heavy", "n_soon"]);
    assert_eq!(by(SortBy::Slack), ["n_soon", "n_heavy"]);
}

/// Priority, "prioritize for me": the next list in the viewer's ranking, where the gate that
/// frees someone else's work leads.
#[test]
fn next_ranks_for_the_viewer() {
    let records = support::journey(&format!(
        "- op: create_entity\n  entity: {{key: e_a, name: A}}\n- op: create_entity\n  entity: {{key: e_b, name: B}}\n{}",
        add(&[
            "{key: n_gate_a, id: gate-a, kind: action, title: Gate, participations: {k_owner: [e_a]}}",
            "{key: n_gate_b, id: gate-b, kind: action, title: Gate, participations: {k_owner: [e_b]}}",
            "{key: n_work_b, id: work-b, kind: action, title: Work, requires: [n_gate_a], participations: {k_owner: [e_b]}}",
            "{key: n_work_a, id: work-a, kind: action, title: Work, requires: [n_gate_b], participations: {k_owner: [e_a]}}",
        ])
    ));
    let derive = Derive::test(&records);
    let for_me = NextQuery {
        for_viewer: true,
        ..NextQuery::default()
    };
    assert_eq!(
        derive.next(&NextQuery::default(), &["e_b"]),
        ["n_gate_a", "n_gate_b"]
    );
    assert_eq!(derive.next(&for_me, &["e_b"]), ["n_gate_b", "n_gate_a"]);
}

/// C5, D5: with the acting frontier empty, the next list carries what the journey waits on.
#[test]
fn an_empty_next_list_says_what_the_journey_waits_on() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_work, id: work, kind: action, title: Work}",
        ])),
        "- op: snooze\n  node: n_work\n  until: {date: \"2026-10-08\"}\n",
    );
    let next = Derive::test(&records)
        .journey()
        .next(&NextQuery::default(), &BTreeSet::new());
    let next = next.unwrap();
    assert_eq!(next.items.len(), 0);
    assert!(next.stalled.is_some());
}

/// E4: "mine" is the nodes the viewer's entity participates in, by kind.
#[test]
fn mine_is_where_the_viewer_participates() {
    let decided = Derive::vendor(2);
    let journey = decided.journey();
    let informed = journey.mine(&entities(&["e_stakeholder_a"]), &BTreeSet::new());
    assert_eq!(informed.len(), 6);
    assert!(
        informed
            .iter()
            .all(|entry| entry.kinds == BTreeSet::from([key_kind("k_informed")]))
    );
    let owned = BTreeSet::from([KindKey::owner()]);
    assert_eq!(
        journey.mine(&entities(&["e_stakeholder_a"]), &owned).len(),
        0
    );
    let all = decided.graph.document().nodes.len();
    assert_eq!(journey.mine(&entities(&["e_lead"]), &owned).len(), all);
}

fn key_kind(text: &str) -> KindKey {
    text.parse().unwrap()
}

// Review round 1 regressions.

/// C9, E4, E6: after the owner is merged into another entity, filtering by the old key and
/// "mine" for a viewer still holding it read through to the survivor.
#[test]
fn owner_filters_and_mine_read_through_a_merge() {
    let records = support::vendor_lead_merged();
    let derive = Derive::on(&records, VENDOR, "2026-10-06");
    let all = derive.graph.document().nodes.len();
    let old_owner = ListQuery {
        owner: Some("e_lead".parse().unwrap()),
        ..ListQuery::default()
    };
    assert_eq!(derive.listed(&old_owner, &[]).len(), all);
    assert_eq!(derive.listed(&flag(ListFlag::Mine), &["e_lead"]).len(), all);
    let owned = BTreeSet::from([KindKey::owner()]);
    assert_eq!(
        derive.journey().mine(&entities(&["e_lead"]), &owned).len(),
        all
    );
}
