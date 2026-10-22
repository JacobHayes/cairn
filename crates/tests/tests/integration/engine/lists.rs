//! C9's list (each filter on a fixture state, in-journey search, sorting, paging), C10's next
//! list (rank order, breadcrumbs and terms, re-sorting by one signal, "only mine", by kind, a
//! subtree, "prioritize for me", the stalled diagnostic), and E4's "mine". Derived at
//! 2026-10-06 unless a test says otherwise.
#![cfg(test)]

use crate::engine::support;

use std::collections::BTreeSet;

use cairn_engine::{Derived, DerivedJourney, Graph, Records};
use cairn_schema::{
    DisplayState, IdentityKey, KindKey, ListFlag, ListQuery, NextQuery, NodeKind, SortBy, State,
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
        let viewer = identities(viewer);
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
        let next = self.journey().next(query, &identities(viewer)).unwrap();
        next.items
            .iter()
            .map(|row| row.key.as_str().to_owned())
            .collect()
    }
}

fn identities(keys: &[&str]) -> BTreeSet<IdentityKey> {
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
        owner: Some("who_k3d7qm2xw5tn6pfa".parse().unwrap()),
        ..ListQuery::default()
    };
    assert_eq!(decided.listed(&lead, &[]).len(), all);
    assert_eq!(
        sorted(decided.listed(&flag(ListFlag::Mine), &["who_d4m6uk2eqf5rbhzs"])),
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
            vec!["n_workload_ingest", "n_workload_query"],
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

/// C9, D8: the blocked filter matches the display state, so a group waiting only on its own
/// children is not listed, while a group behind an unsatisfied requirement is.
#[test]
fn blocked_filter_lists_gated_containers_and_not_those_waiting_on_children() {
    let records = support::accepted_on(
        &support::vendor_after(1),
        VENDOR,
        &(support::add_nodes(&[
            "{key: n_free, id: free, kind: group, title: Free}",
            "{key: n_free_work, id: free_work, parent: n_free, kind: action, title: Free work}",
            "{key: n_gated, id: gated, kind: group, title: Gated}",
            "{key: n_gated_work, id: gated_work, parent: n_gated, kind: action, title: Gated work}",
        ]) + "- op: add_edge\n  edge: {node: n_gated, requires: n_access}\n"),
    );
    let derive = Derive::on(&records, VENDOR, "2026-10-06");
    assert_eq!(
        derive.derived.display_state(&derive.graph, &key("n_free")),
        DisplayState::Ready
    );
    let blocked = derive.listed(&flag(ListFlag::Blocked), &[]);
    assert!(!blocked.contains(&"n_free".to_owned()), "{blocked:?}");
    assert!(blocked.contains(&"n_gated".to_owned()), "{blocked:?}");
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
    assert_eq!(
        Derive::vendor(5).listed(&done, &[]),
        ["n_access", "n_kickoff"]
    );
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

const PAGE: usize = cairn_schema::limits::PAGE_ITEM_COUNT_MAX as usize;

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
    assert_eq!((first.rows.len(), first.total as usize), (PAGE, PAGE + 5));
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
    // The decision meeting is pinned to a day ahead: scheduled, off the acting frontier.
    assert_eq!(keys, ["n_access", "n_workload"]);
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
        decided.next(&mine, &["who_k3d7qm2xw5tn6pfa"]),
        ["n_kickoff"]
    );
    assert_eq!(decided.next(&mine, &["who_d4m6uk2eqf5rbhzs"]).len(), 0);
}

/// C10: re-sorting by one signal: a heavy action with no deadline outranks nothing urgent, but
/// a milestone pinned to today ranks first; by gravity the heavy action leads, by slack the
/// milestone.
#[test]
fn next_re_sorts_by_one_signal() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_heavy, id: heavy, kind: action, title: Heavy, weight: 10}",
            "{key: n_soon, id: soon, kind: milestone, title: Soon}",
        ])),
        "- op: set_pin\n  node: n_soon\n  date: \"2026-10-06\"\n",
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

/// C10: the product launch at the scenario tests' day, where every slack is past the urgency
/// window: the pinned launch and the announcement, held by its own not-before date, wait for
/// their dates off the next list; gravity and unlocks rank the rest, so the documentation
/// leads, and the retrospective, with no deadline, sorts last by slack too.
#[test]
fn the_launch_ranks_by_gravity_at_the_scenario_day_and_re_sorts_by_slack() {
    let launch = Derive::on(
        &support::finished("product-launch"),
        "j_launch",
        "2026-10-06",
    );
    assert_eq!(
        launch.next(&NextQuery::default(), &[]),
        ["n_docs", "n_beta_end", "n_retro"]
    );
    let by_slack = NextQuery {
        sort: SortBy::Slack,
        ..NextQuery::default()
    };
    assert_eq!(
        launch.next(&by_slack, &[]),
        ["n_docs", "n_beta_end", "n_retro"]
    );
}

/// Priority, "prioritize for me": the next list in the viewer's ranking, where the gate that
/// frees someone else's work leads.
#[test]
fn next_ranks_for_the_viewer() {
    let records = support::journey(&format!(
        "- op: create_identity\n  identity: {{key: who_a, name: A}}\n- op: create_identity\n  identity: {{key: who_b, name: B}}\n{}",
        add(&[
            "{key: n_gate_a, id: gate-a, kind: action, title: Gate, participations: {k_owner: [who_a]}}",
            "{key: n_gate_b, id: gate-b, kind: action, title: Gate, participations: {k_owner: [who_b]}}",
            "{key: n_work_b, id: work-b, kind: action, title: Work, requires: [n_gate_a], participations: {k_owner: [who_b]}}",
            "{key: n_work_a, id: work-a, kind: action, title: Work, requires: [n_gate_b], participations: {k_owner: [who_a]}}",
        ])
    ));
    let derive = Derive::test(&records);
    let for_me = NextQuery {
        for_viewer: true,
        ..NextQuery::default()
    };
    assert_eq!(
        derive.next(&NextQuery::default(), &["who_b"]),
        ["n_gate_a", "n_gate_b"]
    );
    assert_eq!(derive.next(&for_me, &["who_b"]), ["n_gate_b", "n_gate_a"]);
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

/// E4: "mine" is the nodes the viewer's identity participates in, by kind.
#[test]
fn mine_is_where_the_viewer_participates() {
    let decided = Derive::vendor(2);
    let journey = decided.journey();
    let informed = journey.mine(&identities(&["who_d4m6uk2eqf5rbhzs"]), &BTreeSet::new());
    assert_eq!(informed.len(), 6);
    assert!(
        informed
            .iter()
            .all(|entry| entry.kinds == BTreeSet::from([key_kind("k_informed")]))
    );
    let owned = BTreeSet::from([KindKey::owner()]);
    assert_eq!(
        journey
            .mine(&identities(&["who_d4m6uk2eqf5rbhzs"]), &owned)
            .len(),
        0
    );
    let all = decided.graph.document().nodes.len();
    assert_eq!(
        journey
            .mine(&identities(&["who_k3d7qm2xw5tn6pfa"]), &owned)
            .len(),
        all
    );
}

/// E4, E6: one route's role is filled by a different person in each journey made from it,
/// so "mine" for a person differs by journey: Ana owns the first evaluation, Dara the
/// second, and neither is the other's.
#[test]
fn mine_follows_who_fills_the_role_in_each_journey() {
    let ana = crate::people::keys::ANA;
    let dara = crate::people::keys::DARA;
    let first = support::vendor_after(2);
    let both = support::accepted_on(
        &first,
        "j_second",
        "- op: create_journey\n  name: Second evaluation\n  from: {route: vendor-evaluation, version: 1}\n",
    );
    let both = support::accepted_on(
        &both,
        "j_second",
        &format!("- op: answer\n  decision: n_who_owns\n  value: {{identity: {dara}}}\n"),
    );
    let owned = BTreeSet::from([KindKey::owner()]);
    let mine = |journey: &str, person: &str| {
        let derive = Derive::on(&both, journey, "2026-10-06");
        derive.journey().mine(&identities(&[person]), &owned).len()
    };
    let first_nodes = both.journeys[&VENDOR.parse().unwrap()].graph.nodes.len();
    let second_nodes = both.journeys[&"j_second".parse().unwrap()]
        .graph
        .nodes
        .len();
    assert_eq!(mine(VENDOR, ana), first_nodes);
    assert_eq!(
        mine(VENDOR, dara),
        0,
        "Dara owns nothing in Ana's evaluation"
    );
    assert_eq!(mine("j_second", dara), second_nodes);
    assert_eq!(
        mine("j_second", ana),
        0,
        "Ana owns nothing in Dara's evaluation"
    );
}

fn key_kind(text: &str) -> KindKey {
    text.parse().unwrap()
}

// Review round 1 regressions.

/// C9, E4, E6: after the owner is merged into another identity, filtering by the old key and
/// "mine" for a viewer still holding it read through to the survivor.
#[test]
fn owner_filters_and_mine_read_through_a_merge() {
    let records = support::vendor_lead_merged();
    let derive = Derive::on(&records, VENDOR, "2026-10-06");
    let all = derive.graph.document().nodes.len();
    let old_owner = ListQuery {
        owner: Some("who_k3d7qm2xw5tn6pfa".parse().unwrap()),
        ..ListQuery::default()
    };
    assert_eq!(derive.listed(&old_owner, &[]).len(), all);
    assert_eq!(
        derive
            .listed(&flag(ListFlag::Mine), &["who_k3d7qm2xw5tn6pfa"])
            .len(),
        all
    );
    let owned = BTreeSet::from([KindKey::owner()]);
    assert_eq!(
        derive
            .journey()
            .mine(&identities(&["who_k3d7qm2xw5tn6pfa"]), &owned)
            .len(),
        all
    );
}
