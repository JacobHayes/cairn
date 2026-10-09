//! C2, semantic zoom: the level projection on the vendor evaluation (each kind toggle's visible
//! set, hoisting under a hidden root, roll-ups, edges re-targeted with duplicates collapsed and
//! self-edges dropped, the hidden-prerequisites marker, drill-in) and on hand-built journeys
//! (each badge, a group's display state, kept work pending). Derived at 2026-10-06.
#![cfg(test)]

use crate::support;

use std::collections::{BTreeMap, BTreeSet};

use cairn_engine::{DerivedJourney, ProjectionError, Records};
use cairn_schema::{
    EdgeOrigin, GroupState, Level, LevelDisplay, LevelNode, LevelQuery, NodeKind, Score,
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
        .level(query, &records.deployment)
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
    let opening = whole
        .edges
        .iter()
        .find(|edge| edge.to.as_str() == "n_setup");
    let opening = opening.unwrap();
    assert!(
        opening.implicit && opening.gates,
        "an opening is a dotted gate"
    );
    assert_eq!(opening.underlying[0].origin, EdgeOrigin::StageOpening);
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
        node(&decided, "n_partner_led").group_state,
        Some(GroupState::NotRelevant),
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
            &records.deployment
        ),
        Err(ProjectionError::UnknownNode(key("n_nowhere")))
    );
}

const GROUP: &str = "{key: n_group, id: group, kind: group, title: Group}";
const ASK: &str = "{key: n_ask, id: ask, parent: n_group, kind: decision, title: Ask, prompt: Go?, answer_type: boolean}";
const WORK: &str = "{key: n_work, id: work, parent: n_group, kind: deliverable, title: Work, participations: {k_owner: [e_a]}}";
const STEP: &str = "{key: n_step, id: step, parent: n_work, kind: action, title: Step}";
const TODO: &str = "{key: n_todo, id: todo, parent: n_group, kind: deliverable, title: To do, placeholder: true, participations: {k_owner: [e_b]}}";
const PEOPLE: &str = "- op: create_entity\n  entity: {key: e_a, name: A}\n- op: create_entity\n  entity: {key: e_b, name: B}\n";

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
    assert!(badges.decision_needed && badges.needs_breakdown && !badges.children_active);
    let owners: Vec<&str> = badges
        .owners
        .iter()
        .map(cairn_schema::EntityKey::as_str)
        .collect();
    assert_eq!(owners, ["e_a", "e_b"]);
    assert_eq!(
        badges.max_child_gravity,
        Some(Score::from_millionths(1_000_000))
    );
    assert!(!badges.all_blocked, "the decision is open to act on");
    assert_eq!(group.group_state, Some(GroupState::NotStarted));
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
    assert_eq!(
        node(&active, "n_group").group_state,
        Some(GroupState::Active)
    );

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

/// C2: `all_blocked` holds when every relevant child not done is blocked, and a group whose
/// opening is ahead reads as waiting.
#[test]
fn every_open_child_blocked_shows_all_blocked() {
    let records = support::journey(&add(&[
        "{key: n_gate, id: gate, kind: milestone, title: Gate}",
        "{key: n_stage, id: stage, kind: group, title: Stage, opens_at: n_gate}",
        "{key: n_one, id: one, parent: n_stage, kind: action, title: One}",
        "{key: n_two, id: two, parent: n_stage, kind: action, title: Two}",
    ]));
    let waiting = level(&records, support::JOURNEY, &NodeKind::ALL, None);
    let stage = node(&waiting, "n_stage");
    assert!(stage.roll_up.as_ref().unwrap().all_blocked);
    assert_eq!(stage.group_state, Some(GroupState::Waiting));
    let opened = support::accepted(&records, &transition("n_gate", "reach"));
    let open = level(&opened, support::JOURNEY, &NodeKind::ALL, None);
    assert!(!node(&open, "n_stage").roll_up.as_ref().unwrap().all_blocked);
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
        node(&pending, "n_side").group_state,
        Some(GroupState::Skipped)
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
