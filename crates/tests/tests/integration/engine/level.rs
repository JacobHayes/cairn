//! C2, semantic zoom: the level projection on the vendor evaluation (each kind toggle's visible
//! set, hoisting under a hidden root, roll-ups, edges re-targeted with duplicates collapsed and
//! self-edges dropped, the hidden-prerequisites marker, drill-in) and on hand-built journeys
//! (each badge, a group's display state, kept work pending). Derived at 2026-10-06.
#![cfg(test)]

use crate::engine::support;

use std::collections::{BTreeMap, BTreeSet};

use cairn_engine::{DerivedJourney, ProjectionError, Records};
use cairn_schema::{
    DateLink, DateLinkKind, DisplayState, Level, LevelDateLink, LevelDisplay, LevelNode,
    LevelQuery, NodeKind,
};
use support::{add_nodes as add, key};

const VENDOR: &str = "j_vendor_eval";

/// The level of a journey in the records for the shown kinds, drilled into `container`.
fn level(records: &Records, journey: &str, shown: &[NodeKind], container: Option<&str>) -> Level {
    let shown: BTreeSet<NodeKind> = shown.iter().copied().collect();
    level_of(
        records,
        journey,
        &LevelQuery::of_kinds(shown, container.map(key)),
    )
}

/// The level of a journey in the records for a query.
fn level_of(records: &Records, journey: &str, query: &LevelQuery) -> Level {
    let graph = support::journey_graph(records, journey);
    let derived = support::derived(records, journey);
    DerivedJourney::new(&graph, &derived)
        .level(query, &records.deployment, &BTreeSet::new())
        .unwrap()
}

/// Every kind shown, the containers collapsed, and the relevance classes shown.
fn query(collapsed: &[&str], display: &[LevelDisplay]) -> LevelQuery {
    LevelQuery {
        collapsed: collapsed.iter().map(|name| key(name)).collect(),
        display: display.iter().copied().collect(),
        ..LevelQuery::of_kinds(NodeKind::ALL.into_iter().collect(), None)
    }
}

fn every_kind_but(hidden: NodeKind) -> Vec<NodeKind> {
    NodeKind::ALL
        .into_iter()
        .filter(|kind| *kind != hidden)
        .collect()
}

fn node<'a>(level: &'a Level, name: &str) -> &'a LevelNode {
    level
        .nodes
        .iter()
        .find(|node| node.key.as_str() == name)
        .unwrap_or_else(|| panic!("{name} is not visible"))
}

/// Each visible node and the node it is drawn under.
fn placement(level: &Level) -> BTreeMap<&str, Option<&str>> {
    level
        .nodes
        .iter()
        .map(|node| {
            (
                node.key.as_str(),
                node.parent.as_ref().map(cairn_schema::NodeKey::as_str),
            )
        })
        .collect()
}

/// Each drawn edge as (from, to) with how many edges it stands for.
fn edges(level: &Level) -> Vec<(&str, &str, usize)> {
    level
        .edges
        .iter()
        .map(|edge| (edge.from.as_str(), edge.to.as_str(), edge.underlying.len()))
        .collect()
}

fn names(keys: &[cairn_schema::NodeKey]) -> Vec<&str> {
    keys.iter().map(cairn_schema::NodeKey::as_str).collect()
}

/// C1, C3: every kind shown is the whole graph: every node under its own parent and every
/// explicit edge, condition gate, and stage opening drawn as itself.
#[test]
fn every_kind_shown_draws_the_whole_graph() {
    let records = support::vendor_after(1);
    let whole = level(&records, VENDOR, &NodeKind::ALL, None);
    let graph = support::journey_graph(&records, VENDOR);
    assert_eq!(whole.nodes.len(), graph.document().nodes.len());
    for node in &whole.nodes {
        assert_eq!(node.parent.as_ref(), graph.tree().parent(&node.key));
        assert_eq!(node.rolled_up.len(), 0);
    }
    assert_eq!(
        edges(&whole),
        [
            ("n_access", "n_plan", 1),
            ("n_comparison_set", "n_baseline", 1),
            ("n_criteria", "n_partner_results", 1),
            ("n_findings", "n_final_report", 1),
            ("n_findings", "n_findings_reviewer", 1),
            ("n_kickoff", "n_setup", 1),
            ("n_partner_runs", "n_partner_led", 1),
            ("n_plan", "n_comparison_set", 1),
            ("n_plan_draft", "n_plan_review", 1),
            ("n_review_opens", "n_final_review", 1),
            ("n_testing", "n_reporting", 1),
        ]
    );
}

/// C2, C4: with actions hidden, each rolls up into its deliverable or group as a checklist
/// item, and an edge between two actions of one deliverable lands on it and is not drawn.
#[test]
fn hidden_actions_roll_up_and_their_edge_onto_one_node_is_dropped() {
    let records = support::vendor_after(1);
    let without = level(&records, VENDOR, &every_kind_but(NodeKind::Action), None);
    assert_eq!(
        names(&node(&without, "n_plan").rolled_up),
        ["n_plan_draft", "n_plan_review"]
    );
    assert_eq!(
        names(&node(&without, "n_partner_led").rolled_up),
        ["n_criteria", "n_partner_results"]
    );
    let drawn = edges(&without);
    assert_eq!(drawn.len(), 9, "{drawn:?}");
    assert!(drawn.iter().all(|(from, to, _)| from != to));
    assert!(!drawn.iter().any(|(from, _, _)| *from == "n_plan_draft"));
}

/// C2: the scenario's end at the Work step (stages, decisions, deliverables, milestones): each
/// node is drawn under its container (a node settled not relevant too: the page hides it), and
/// the hidden actions are in their deliverable's progress.
#[test]
fn the_scenarios_end_at_the_work_step_draws_each_node_in_its_container() {
    let records = support::finished("vendor-evaluation");
    let work = level(
        &records,
        VENDOR,
        &[
            NodeKind::Group,
            NodeKind::Decision,
            NodeKind::Deliverable,
            NodeKind::Milestone,
        ],
        None,
    );
    let top = [
        "n_decision_meeting",
        "n_kickoff",
        "n_meeting_date",
        "n_partner_runs",
        "n_purpose",
        "n_reporting",
        "n_setup",
        "n_testing",
        "n_who_informed",
        "n_who_owns",
    ];
    let inside = [
        ("n_final_review", "n_reporting"),
        ("n_final_report", "n_final_review"),
        ("n_findings", "n_reporting"),
        ("n_findings_reviewer", "n_reporting"),
        ("n_review_opens", "n_reporting"),
        ("n_access", "n_setup"),
        ("n_plan", "n_setup"),
        ("n_workload", "n_setup"),
        ("n_workload_ingest", "n_workload"),
        ("n_workload_query", "n_workload"),
        ("n_baseline", "n_testing"),
        ("n_comparison_set", "n_testing"),
        ("n_partner_led", "n_testing"),
    ];
    let expected: BTreeMap<&str, Option<&str>> = top
        .into_iter()
        .map(|node| (node, None))
        .chain(
            inside
                .into_iter()
                .map(|(node, parent)| (node, Some(parent))),
        )
        .collect();
    assert_eq!(placement(&work), expected);
    assert_eq!(names(&node(&work, "n_plan").rolled_up).len(), 2);
}

/// C2: with groups hidden, the roots Setup, Testing, and Reporting are hidden, so their
/// contents are drawn at the top level; the openings and the requirement on Reporting have no
/// visible stand-in and are left off, so every node they block carries the hidden-prerequisites
/// marker naming what it waits on.
#[test]
fn a_hidden_root_hoists_its_contents_and_marks_what_its_gates_block() {
    let records = support::vendor_after(1);
    let hoisted = level(&records, VENDOR, &every_kind_but(NodeKind::Group), None);
    let placed = placement(&hoisted);
    for top in [
        "n_access",
        "n_plan",
        "n_workload",
        "n_baseline",
        "n_findings",
    ] {
        assert_eq!(placed[top], None, "{top} is drawn at the top level");
    }
    assert_eq!(placed["n_plan_draft"], Some("n_plan"));
    assert!(!placed.contains_key("n_setup"));
    assert!(
        !edges(&hoisted)
            .iter()
            .any(|(from, _, _)| *from == "n_kickoff")
    );
    for under_setup in ["n_access", "n_plan", "n_plan_draft", "n_workload"] {
        assert_eq!(
            names(&node(&hoisted, under_setup).hidden_prerequisites),
            ["n_kickoff"],
            "{under_setup} waits on the hidden stage's opening"
        );
    }
    assert_eq!(
        names(&node(&hoisted, "n_final_report").hidden_prerequisites),
        ["n_review_opens", "n_testing"]
    );
    assert_eq!(
        node(&hoisted, "n_baseline").hidden_prerequisites.len(),
        0,
        "its condition gate is drawn"
    );
    assert_eq!(
        node(&hoisted, "n_decision_meeting")
            .hidden_prerequisites
            .len(),
        0
    );
}

/// C2: a visible action whose blocking decision is hidden carries the marker; so does the
/// group the decision's condition sits on.
#[test]
fn a_visible_action_behind_a_hidden_decision_is_marked() {
    let records = support::vendor_after(1);
    let without = level(&records, VENDOR, &every_kind_but(NodeKind::Decision), None);
    for marked in ["n_criteria", "n_partner_led"] {
        assert_eq!(
            names(&node(&without, marked).hidden_prerequisites),
            ["n_partner_runs"],
            "{marked}"
        );
    }
    let decided = support::vendor_after(2);
    let after = level(&decided, VENDOR, &every_kind_but(NodeKind::Decision), None);
    assert!(
        after
            .nodes
            .iter()
            .all(|node| !node.hidden_prerequisites.contains(&key("n_partner_runs"))),
        "an answered decision blocks nothing"
    );
}

/// C2: with only groups shown, two edges land on the same pair of groups (the findings into
/// the final report, and the review opening into the final review) and collapse into one edge
/// that stands for both; edges inside one group are not drawn.
#[test]
fn edges_landing_on_the_same_pair_collapse_into_one() {
    let records = support::vendor_after(1);
    let groups = level(&records, VENDOR, &[NodeKind::Group], None);
    assert_eq!(
        placement(&groups),
        BTreeMap::from([
            ("n_final_review", Some("n_reporting")),
            ("n_partner_led", Some("n_testing")),
            ("n_reporting", None),
            ("n_setup", None),
            ("n_testing", None),
        ])
    );
    assert_eq!(
        edges(&groups),
        [
            ("n_reporting", "n_final_review", 2),
            ("n_setup", "n_testing", 1),
            ("n_testing", "n_reporting", 1),
        ]
    );
    let collapsed = &groups.edges[0];
    assert!(!collapsed.implicit, "one of the two is explicit");
    let decided = level(&support::vendor_after(2), VENDOR, &[NodeKind::Group], None);
    assert_eq!(
        node(&decided, "n_partner_led").display_state,
        DisplayState::NotRelevant,
        "a group the partner decision ruled out"
    );
}

/// C2, C4: drilled into Setup, only its contents are visible, at the top of the sub-canvas;
/// the opening from outside has no stand-in, so before kickoff each blocked node is marked.
#[test]
fn drilling_in_shows_the_containers_contents() {
    let records = support::vendor_after(1);
    let setup = level(&records, VENDOR, &NodeKind::ALL, Some("n_setup"));
    assert_eq!(
        placement(&setup),
        BTreeMap::from([
            ("n_access", None),
            ("n_plan", None),
            ("n_plan_draft", Some("n_plan")),
            ("n_plan_review", Some("n_plan")),
            ("n_workload", None),
        ])
    );
    assert_eq!(
        edges(&setup),
        [
            ("n_access", "n_plan", 1),
            ("n_plan_draft", "n_plan_review", 1)
        ]
    );
    assert_eq!(
        names(&node(&setup, "n_access").hidden_prerequisites),
        ["n_kickoff"]
    );
    let kicked_off = level(
        &support::vendor_after(3),
        VENDOR,
        &NodeKind::ALL,
        Some("n_setup"),
    );
    assert_eq!(node(&kicked_off, "n_access").hidden_prerequisites.len(), 0);
    let graph = support::journey_graph(&records, VENDOR);
    let derived = support::derived(&records, VENDOR);
    assert_eq!(
        DerivedJourney::new(&graph, &derived).level(
            &LevelQuery::of_kinds(BTreeSet::new(), Some(key("n_nowhere"))),
            &records.deployment,
            &BTreeSet::new()
        ),
        Err(ProjectionError::UnknownNode(key("n_nowhere")))
    );
}

const GROUP: &str = "{key: n_group, id: group, kind: group, title: Group}";
const ASK: &str = "{key: n_ask, id: ask, parent: n_group, kind: decision, title: Ask, prompt: Go?, answer_type: boolean}";
const WORK: &str = "{key: n_work, id: work, parent: n_group, kind: deliverable, title: Work, participations: {k_owner: [who_a]}}";
const STEP: &str = "{key: n_step, id: step, parent: n_work, kind: action, title: Step}";
const TODO: &str = "{key: n_todo, id: todo, parent: n_group, kind: deliverable, title: To do, placeholder: true, participations: {k_owner: [who_b]}}";
const PEOPLE: &str = "- op: create_identity\n  identity: {key: who_a, name: A}\n- op: create_identity\n  identity: {key: who_b, name: B}\n";

fn transition(node: &str, transition: &str) -> String {
    format!("- op: transition\n  node: {node}\n  transition: {transition}\n")
}

/// C2: a container's badges: a decision needed, a placeholder needing breakdown, its
/// children's distinct owners, and its largest child gravity; then a child active; then a
/// deliverable whose children are done reads "ready to finish". A group's display state moves
/// from not started to active to done.
#[test]
fn a_container_rolls_up_its_childrens_badges() {
    let records = support::journey(&format!("{PEOPLE}{}", add(&[GROUP, ASK, WORK, STEP, TODO])));
    let fresh = level(&records, support::JOURNEY, &NodeKind::ALL, None);
    let group = node(&fresh, "n_group");
    let badges = group.roll_up.as_ref().unwrap();
    assert!(
        badges.progress.decisions_needed > 0 && badges.needs_breakdown && !badges.children_active
    );
    let owners: Vec<&str> = badges
        .owners
        .iter()
        .map(cairn_schema::IdentityKey::as_str)
        .collect();
    assert_eq!(owners, ["who_a", "who_b"]);
    assert!(!badges.all_blocked, "the decision is open to act on");
    assert_eq!(group.display_state, DisplayState::Ready);
    assert!(
        !node(&fresh, "n_work")
            .roll_up
            .as_ref()
            .unwrap()
            .ready_to_finish
    );

    let started = support::accepted(&records, &transition("n_step", "start"));
    let active = level(&started, support::JOURNEY, &NodeKind::ALL, None);
    assert!(
        node(&active, "n_work")
            .roll_up
            .as_ref()
            .unwrap()
            .children_active
    );
    assert_eq!(node(&active, "n_group").display_state, DisplayState::Active);

    let stepped = support::accepted(&started, &transition("n_step", "complete"));
    let ready = level(&stepped, support::JOURNEY, &NodeKind::ALL, None);
    assert!(
        node(&ready, "n_work")
            .roll_up
            .as_ref()
            .unwrap()
            .ready_to_finish
    );
}

/// C2, D8: `all_blocked` holds when every relevant child not done shows blocked, and a group
/// whose opening is ahead shows blocked; a child container waiting only on its own children
/// is not blocked, so it never makes its parent read as all blocked.
#[test]
fn every_open_child_blocked_shows_all_blocked() {
    let records = support::journey(&add(&[
        "{key: n_gate, id: gate, kind: milestone, title: Gate}",
        "{key: n_stage, id: stage, kind: group, title: Stage, requires: [n_gate]}",
        "{key: n_one, id: one, parent: n_stage, kind: action, title: One}",
        "{key: n_two, id: two, parent: n_stage, kind: action, title: Two}",
    ]));
    let waiting = level(&records, support::JOURNEY, &NodeKind::ALL, None);
    let stage = node(&waiting, "n_stage");
    assert!(stage.roll_up.as_ref().unwrap().all_blocked);
    assert_eq!(stage.display_state, DisplayState::Blocked);
    let opened = support::accepted(&records, &transition("n_gate", "complete"));
    let open = level(&opened, support::JOURNEY, &NodeKind::ALL, None);
    assert!(!node(&open, "n_stage").roll_up.as_ref().unwrap().all_blocked);

    let nested = support::journey(&add(&[
        "{key: n_area, id: area, kind: group, title: Area}",
        "{key: n_part, id: part, parent: n_area, kind: deliverable, title: Part}",
        "{key: n_piece, id: piece, parent: n_part, kind: action, title: Piece}",
    ]));
    let shown = level(&nested, support::JOURNEY, &NodeKind::ALL, None);
    assert_eq!(node(&shown, "n_part").display_state, DisplayState::Ready);
    assert!(!node(&shown, "n_area").roll_up.as_ref().unwrap().all_blocked);
}

/// D1a: a skipped deliverable with kept work beneath it reads "skipped, kept work pending"
/// until the kept work is done; a skipped group reads as skipped.
#[test]
fn a_skipped_container_shows_kept_work_pending() {
    let records = support::journey(&add(&[
        "{key: n_plan, id: plan, kind: deliverable, title: Plan}",
        "{key: n_draft, id: draft, parent: n_plan, kind: action, title: Draft}",
        "{key: n_side, id: side, kind: group, title: Side}",
        "{key: n_note, id: note, parent: n_side, kind: action, title: Note}",
    ]));
    let skipped = support::accepted(
        &records,
        "- op: apply_override\n  node: n_draft\n  override: {keep: {reason: Still needed.}}\n- op: transition\n  node: n_plan\n  transition: {skip: {reason: Not needed.}}\n- op: transition\n  node: n_side\n  transition: {skip: {reason: Not needed.}}\n",
    );
    let pending = level(&skipped, support::JOURNEY, &NodeKind::ALL, None);
    assert!(node(&pending, "n_plan").kept_work_pending);
    assert_eq!(
        node(&pending, "n_side").display_state,
        DisplayState::Skipped
    );
    let kept_done = support::accepted(&skipped, &transition("n_draft", "complete"));
    let done = level(&kept_done, support::JOURNEY, &NodeKind::ALL, None);
    assert!(!node(&done, "n_plan").kept_work_pending);
}

/// C2: a collapsed stage rolls its subtree into its card; edges into it and out of it re-target
/// to the card, two edges landing on the same pair of cards merge into one that stands for both,
/// and edges inside the stage are not drawn.
#[test]
fn a_collapsed_stage_re_targets_its_edges_and_merges_duplicates() {
    let records = support::vendor_patch(
        &support::vendor_after(1),
        &support::add_nodes(&[
            "{key: n_extra, id: extra, parent: n_testing, kind: action, title: Extra, requires: [n_access]}",
        ]),
    )
    .unwrap();
    let level = level_of(
        records.records(),
        VENDOR,
        &query(&["n_setup", "n_testing"], &LevelDisplay::ALL),
    );
    assert_eq!(
        names(&node(&level, "n_setup").rolled_up),
        [
            "n_access",
            "n_plan",
            "n_plan_draft",
            "n_plan_review",
            "n_workload"
        ]
    );
    assert!(
        !level
            .nodes
            .iter()
            .any(|node| node.key.as_str() == "n_criteria")
    );
    assert_eq!(
        edges(&level)
            .into_iter()
            .filter(|(from, to, _)| ["n_setup", "n_testing"].contains(from)
                || ["n_setup", "n_testing"].contains(to))
            .collect::<Vec<_>>(),
        [
            // into the stage (a gate from outside, and a decision's condition),
            ("n_kickoff", "n_setup", 1),
            ("n_partner_runs", "n_testing", 1),
            // between the stages, two edges merged (the plan, and access into Extra),
            ("n_setup", "n_testing", 2),
            // and out of a stage.
            ("n_testing", "n_reporting", 1),
        ]
    );
    assert_eq!(level.collapsed.len(), 2);
}

/// C2: nodes ruled out by an answer, hidden by the display set, roll up like any hidden node and
/// leave no orphan line: an edge into one lands on its nearest visible ancestor, and a hidden
/// top-level node's edge, having no stand-in, is not drawn.
#[test]
fn hidden_not_relevant_nodes_leave_no_orphan_lines() {
    let ruled_out = support::vendor_patch(
        &support::vendor_after(2),
        &support::add_nodes(&[
            "{key: n_side, id: side, kind: action, title: Side, relevant_when: {equals: {decision: n_partner_runs, value: true}}}",
        ]),
    )
    .unwrap();
    let records = ruled_out.records();
    let hidden = level_of(
        records,
        VENDOR,
        &query(&[], &[LevelDisplay::Relevant, LevelDisplay::Conditional]),
    );
    let drawn: BTreeSet<&str> = hidden.nodes.iter().map(|node| node.key.as_str()).collect();
    assert!(!drawn.contains("n_side") && !drawn.contains("n_partner_led"));
    assert_eq!(
        names(&node(&hidden, "n_testing").rolled_up),
        ["n_partner_led", "n_criteria", "n_partner_results"]
    );
    for (from, to, _) in edges(&hidden) {
        assert!(drawn.contains(from) && drawn.contains(to), "{from} to {to}");
    }
    assert!(edges(&hidden).contains(&("n_partner_runs", "n_testing", 1)));
    assert!(!edges(&hidden).iter().any(|(_, to, _)| *to == "n_side"));
}

/// C2: a node that is not relevant only until a decision upstream of it is answered shows with
/// the conditional ones, and is not counted among the settled ones: leaving conditional out
/// hides it with the undecided, and leaving not relevant out does not.
#[test]
fn pending_nodes_travel_with_the_conditional_ones() {
    use LevelDisplay::{Conditional, NotRelevant, Relevant};
    let chain = support::journey(&support::add_nodes(&[
        "{key: n_d1, id: d1, kind: decision, title: D1, prompt: D1?, answer_type: boolean}",
        "{key: n_d2, id: d2, kind: decision, title: D2, prompt: D2?, answer_type: boolean, relevant_when: {equals: {decision: n_d1, value: true}}}",
        "{key: n_x, id: x, kind: action, title: X, relevant_when: {equals: {decision: n_d2, value: true}}}",
    ]));
    let visible = |records: &Records, display: &[LevelDisplay]| -> Vec<String> {
        level_of(records, support::JOURNEY, &query(&[], display))
            .nodes
            .iter()
            .map(|node| node.key.to_string())
            .collect()
    };
    assert_eq!(visible(&chain, &[Relevant, NotRelevant]), ["n_d1"]);
    assert_eq!(
        visible(&chain, &[Relevant, Conditional]),
        ["n_d1", "n_d2", "n_x"]
    );
    let no = support::accepted(
        &chain,
        "- op: answer\n  decision: n_d1\n  value: {boolean: false}\n",
    );
    assert_eq!(visible(&no, &[Relevant, Conditional]), ["n_d1"]);
    assert_eq!(
        visible(&no, &[Relevant, NotRelevant]),
        ["n_d1", "n_d2", "n_x"]
    );
}

/// Each date link as (from, to, orders, what it stands for), the latter as `node source kind`
/// per relationship, sorted.
fn date_links(links: &[LevelDateLink]) -> Vec<(String, String, bool, Vec<String>)> {
    links
        .iter()
        .map(|link| {
            (
                link.from.as_str().to_owned(),
                link.to.as_str().to_owned(),
                link.orders,
                link.links.iter().map(describe).collect(),
            )
        })
        .collect()
}

/// Every date link of a level, gathered from the levels that select each of its cards in
/// turn: a level carries the links around its selected card only.
fn every_date_link(records: &Records, journey: &str, shown: &[NodeKind]) -> Vec<LevelDateLink> {
    let shown: BTreeSet<NodeKind> = shown.iter().copied().collect();
    let cards = level_of(records, journey, &LevelQuery::of_kinds(shown.clone(), None));
    let mut found: Vec<LevelDateLink> = cards
        .nodes
        .iter()
        .flat_map(|card| {
            let selecting = LevelQuery {
                selected: Some(card.key.clone()),
                ..LevelQuery::of_kinds(shown.clone(), None)
            };
            level_of(records, journey, &selecting).date_links
        })
        .collect();
    found.sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));
    found.dedup();
    found
}

fn describe(link: &DateLink) -> String {
    let kind = match &link.kind {
        DateLinkKind::Rule {
            rule,
            direction,
            days,
            answer,
        } => format!(
            "{rule:?} {direction:?} {days}{}",
            if *answer { " answer" } else { "" }
        ),
        DateLinkKind::FedDate => "fed".to_owned(),
    };
    format!("{} {} {kind}", link.node, link.source)
}

/// One link standing for one relationship.
fn single(from: &str, to: &str, orders: bool, link: &str) -> (String, String, bool, Vec<String>) {
    (
        from.to_owned(),
        to.to_owned(),
        orders,
        vec![link.to_owned()],
    )
}

/// C1: the launch fixture's date relationships, each with its direction by meaning and whether
/// it fixes an order: a due-before runs work to the milestone it
/// is due before, a not-before runs the milestone to the work, and a due-after, which fixes no
/// order, is drawn from its source to its node.
#[test]
fn date_links_say_their_direction_and_whether_they_fix_an_order() {
    let records = support::finished("product-launch");
    let mut found = date_links(&every_date_link(&records, "j_launch", &NodeKind::ALL));
    found.sort();
    let rule = |node: &str, source: &str, what: &str| format!("{node} {source} {what}");
    let expected = [
        single(
            "n_build",
            "n_code_freeze",
            true,
            &rule("n_build", "n_code_freeze", "DueBy Before 0"),
        ),
        single(
            "n_beta",
            "n_beta_end",
            true,
            &rule("n_beta", "n_beta_end", "DueBy Before 0"),
        ),
        single(
            "n_code_freeze",
            "n_launch",
            true,
            &rule("n_code_freeze", "n_launch", "DueBy Before 21"),
        ),
        single(
            "n_docs",
            "n_launch",
            true,
            &rule("n_docs", "n_launch", "DueBy Before 7"),
        ),
        single(
            "n_announcement",
            "n_launch",
            true,
            &rule("n_announcement", "n_launch", "DueBy Before 3"),
        ),
        single(
            "n_go_no_go",
            "n_launch",
            true,
            &rule("n_go_no_go", "n_launch", "DueBy Before 1"),
        ),
        single(
            "n_code_freeze",
            "n_beta_start",
            true,
            &rule("n_beta_start", "n_code_freeze", "NotBefore After 2"),
        ),
        single(
            "n_beta_end",
            "n_announcement",
            true,
            &rule("n_announcement", "n_beta_end", "NotBefore After 0"),
        ),
        single(
            "n_launch",
            "n_retro",
            true,
            &rule("n_retro", "n_launch", "NotBefore After 5"),
        ),
        single(
            "n_beta_start",
            "n_beta_end",
            false,
            &rule("n_beta_end", "n_beta_start", "DueBy After 10"),
        ),
    ];
    let mut expected: Vec<_> = expected.into_iter().collect();
    expected.sort();
    assert_eq!(found, expected);
}

/// Every date link of the test journey's whole-graph level for inline nodes.
fn journey_level(nodes: &[&str]) -> Vec<LevelDateLink> {
    let records = support::journey(&support::add_nodes(nodes));
    every_date_link(&records, support::JOURNEY, &NodeKind::ALL)
}

/// C1, A8: the signed matrix the launch fixture lacks. A zero offset fixes the order in
/// either direction; a due-after or a not-before-before with days does not; a date answer as
/// a source never does; a decision that sets a milestone's date is a link that fixes none.
#[test]
fn date_links_follow_the_signed_offsets_and_answers() {
    let level = journey_level(&[
        "{key: n_m, id: m, kind: milestone, title: M}",
        "{key: n_d, id: d, kind: decision, title: D, prompt: P, answer_type: date, feeds_milestone: n_m}",
        "{key: n_a, id: a, kind: action, title: A, due_by: {after: n_m}}",
        "{key: n_b, id: b, kind: action, title: B, due_by: {before: n_m}}",
        "{key: n_c, id: c, kind: action, title: C, not_before: {before: n_m}}",
        "{key: n_e, id: e, kind: action, title: E, not_before: {before: n_m, offset: 3}}",
        "{key: n_f, id: f, kind: action, title: F, due_by: {before: n_d, offset: 2}}",
    ]);
    let mut found = date_links(&level);
    found.sort();
    let mut expected = vec![
        single("n_a", "n_m", true, "n_a n_m DueBy After 0"),
        single("n_b", "n_m", true, "n_b n_m DueBy Before 0"),
        single("n_m", "n_c", true, "n_c n_m NotBefore Before 0"),
        single("n_m", "n_e", false, "n_e n_m NotBefore Before 3"),
        single("n_d", "n_f", false, "n_f n_d DueBy Before 2 answer"),
        single("n_m", "n_d", false, "n_d n_m fed"),
    ];
    expected.sort();
    assert_eq!(found, expected);
}

/// C1: at Stages, the relationships of hidden work roll up into the card holding it and
/// collapse per pair: two rules into Launch from the materials group make one link that
/// stands for both, the group's own link to nowhere on its own card is dropped, and a link
/// that fixes an order marks the whole pair.
#[test]
fn date_links_roll_up_to_stand_ins_and_collapse() {
    let records = support::finished("product-launch");
    let stages = every_date_link(
        &records,
        "j_launch",
        &[NodeKind::Group, NodeKind::Milestone],
    );
    let links = date_links(&stages);
    let materials = links
        .iter()
        .find(|(from, to, _, _)| (from.as_str(), to.as_str()) == ("n_materials", "n_launch"))
        .expect("the materials group is due before launch");
    assert!(materials.2);
    assert_eq!(
        materials.3,
        [
            "n_announcement n_launch DueBy Before 3",
            "n_docs n_launch DueBy Before 7"
        ]
    );
    assert!(
        links.iter().all(|(from, to, _, _)| from != to),
        "none lands on one card"
    );
    let underlying: usize = stages.iter().map(|link| link.links.len()).sum();
    let whole = every_date_link(&records, "j_launch", &NodeKind::ALL);
    let all: usize = whole.iter().map(|link| link.links.len()).sum();
    assert!(
        underlying < all,
        "the relationships inside a card stand for nothing at this level"
    );
}

/// C1: a pair holding an ordering relationship and one that fixes no order is one link, and
/// it fixes the order.
#[test]
fn a_pair_with_one_ordering_relationship_fixes_the_order() {
    let level = journey_level(&[
        "{key: n_m, id: m, kind: milestone, title: M}",
        "{key: n_a, id: a, kind: action, title: A, due_by: {before: n_m, offset: 2}, not_before: {before: n_m, offset: 4}}",
    ]);
    assert_eq!(
        date_links(&level),
        [(
            "n_a".to_owned(),
            "n_m".to_owned(),
            true,
            vec![
                "n_a n_m DueBy Before 2".to_owned(),
                "n_a n_m NotBefore Before 4".to_owned()
            ]
        )]
    );
}

/// C1, F2: a relationship whose end relevance ruled out is not a link; one resting on an
/// undecided decision is kept.
#[test]
fn a_ruled_out_end_has_no_date_links() {
    let decision = "{key: n_d, id: d, kind: decision, title: D, prompt: P, answer_type: boolean}";
    let nodes = |condition: &str| {
        [
            decision.to_owned(),
            "{key: n_m, id: m, kind: milestone, title: M}".to_owned(),
            format!(
                "{{key: n_a, id: a, kind: action, title: A, due_by: {{before: n_m}}, relevant_when: {condition}}}"
            ),
        ]
    };
    let level_of = |records: &Records| every_date_link(records, support::JOURNEY, &NodeKind::ALL);
    let build = |condition: &str, answer: Option<&str>| {
        let list = nodes(condition);
        let refs: Vec<&str> = list.iter().map(String::as_str).collect();
        let records = support::journey(&support::add_nodes(&refs));
        match answer {
            Some(value) => support::accepted(
                &records,
                &format!("- op: answer\n  decision: n_d\n  value: {{boolean: {value}}}\n"),
            ),
            None => records,
        }
    };
    let condition = "{equals: {decision: n_d, value: true}}";
    let undecided = date_links(&level_of(&build(condition, None)));
    assert_eq!(undecided.len(), 1, "{undecided:?}");
    let ruled_in = date_links(&level_of(&build(condition, Some("true"))));
    assert_eq!(
        ruled_in, undecided,
        "answering it the way it asks changes nothing"
    );
    let ruled_out = date_links(&level_of(&build(condition, Some("false"))));
    assert!(
        ruled_out.is_empty(),
        "the rule's node is ruled out: {ruled_out:?}"
    );
}

/// C8: a node's dates list its direct relationships both ways: the launch milestone's own
/// rules and the ones that read it.
#[test]
fn node_dates_list_direct_links_both_ways() {
    let records = support::finished("product-launch");
    let graph = support::journey_graph(&records, "j_launch");
    let derived = support::derived(&records, "j_launch");
    let launch = derived.to_schema(&graph).nodes[&key("n_launch")]
        .dates
        .links
        .clone();
    let mut found: Vec<String> = launch.iter().map(describe).collect();
    found.sort();
    assert_eq!(
        found,
        [
            "n_announcement n_launch DueBy Before 3",
            "n_code_freeze n_launch DueBy Before 21",
            "n_docs n_launch DueBy Before 7",
            "n_go_no_go n_launch DueBy Before 1",
            "n_retro n_launch NotBefore After 5",
        ]
    );
    assert_eq!(
        derived.node_derived(&graph, &key("n_launch")).dates.links,
        launch,
        "one node's detail reads the same relationships"
    );
}

/// C1: a level carries the date relationships around its selected card only, so a graph full
/// of them costs a level nothing until a card is selected.
#[test]
fn a_level_carries_date_links_around_the_selected_card_only() {
    let records = support::finished("product-launch");
    let kinds: BTreeSet<NodeKind> = NodeKind::ALL.into_iter().collect();
    let unselected = level_of(
        &records,
        "j_launch",
        &LevelQuery::of_kinds(kinds.clone(), None),
    );
    assert_eq!(
        unselected.date_links,
        [] as [cairn_schema::LevelDateLink; 0]
    );
    let selecting = |name: &str| LevelQuery {
        selected: Some(key(name)),
        ..LevelQuery::of_kinds(kinds.clone(), None)
    };
    let launch = level_of(&records, "j_launch", &selecting("n_launch")).date_links;
    assert_ne!(launch, [] as [cairn_schema::LevelDateLink; 0]);
    assert!(
        launch
            .iter()
            .all(|link| link.from.as_str() == "n_launch" || link.to.as_str() == "n_launch")
    );
    let docs = level_of(&records, "j_launch", &selecting("n_docs")).date_links;
    assert_eq!(
        date_links(&docs),
        [single(
            "n_docs",
            "n_launch",
            true,
            "n_docs n_launch DueBy Before 7"
        )]
    );
}
