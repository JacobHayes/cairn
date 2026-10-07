//! Pass 5 (PRD Containment, Gating, D1, D1a, D2, D5, B6, F1): what satisfies dependencies,
//! blocked and actionable, the frontier and the acting frontier, snoozes holding from current
//! state, auto-reach, `stalled`, `needs_breakdown`, and `stale` (D4). Hand-built journeys are
//! created on 2026-10-06 and derived at that day unless a test says otherwise.
#![cfg(test)]

use crate::support;

use std::collections::BTreeSet;

use cairn_engine::{Derived, Graph, Records};
use cairn_schema::{
    Date, DependencyVia, GuardFailure, NodeKey, Relevance, SnoozeTarget, StallCause,
};
use support::{add_nodes as add, key};

fn transition(node: &str, transition: &str) -> String {
    format!("- op: transition\n  node: {node}\n  transition: {transition}\n")
}

fn answer(decision: &str, value: &str) -> String {
    format!("- op: answer\n  decision: {decision}\n  value: {value}\n")
}

fn date(text: &str) -> Date {
    text.parse().unwrap()
}

/// Applies each patch in turn to the test journey.
fn then(records: &Records, patches: &[String]) -> Records {
    patches.iter().fold(records.clone(), |records, patch| {
        support::accepted(&records, patch)
    })
}

/// The test journey derived at `today`.
fn derived_on(records: &Records, today: &str) -> (Graph, Derived) {
    let graph = support::journey_graph(records, support::JOURNEY);
    let mut inputs = cairn_engine::testing::derive_inputs(records.deployment.clone());
    inputs.today = date(today);
    let created_on = records.journeys[&support::JOURNEY.parse().unwrap()]
        .header
        .created_on;
    let derived = cairn_engine::derive(&graph, Some(created_on), &inputs);
    (graph, derived)
}

fn derived(records: &Records) -> Derived {
    derived_on(records, "2026-10-06").1
}

fn names(keys: &[NodeKey]) -> Vec<&str> {
    keys.iter().map(NodeKey::as_str).collect()
}

fn frontier(derived: &Derived) -> Vec<&str> {
    names(derived.blocking().frontier())
}

fn acting(derived: &Derived) -> Vec<&str> {
    names(derived.blocking().acting_frontier())
}

/// The node's own blockers, as (node, via).
fn blockers(derived: &Derived, node: &str) -> BTreeSet<(String, DependencyVia)> {
    derived
        .blocked_by(&key(node))
        .into_iter()
        .map(|blocker| (blocker.node.as_str().to_owned(), blocker.via))
        .collect()
}

fn blocker(node: &str, via: DependencyVia) -> (String, DependencyVia) {
    (node.to_owned(), via)
}

const VENDOR: &str = "j_vendor_eval";

/// Illustrative example, Gating: the up-front decisions are the first frontier; the kickoff
/// milestone gates Setup, so nothing in it is actionable until kickoff; the comparison set
/// waits on the plan, and the baseline, undecided, waits on the comparison set.
#[test]
fn the_vendor_evaluation_yields_the_prds_frontier() {
    let created = support::derived(&support::vendor_after(1), VENDOR);
    let graph = support::journey_graph(&support::vendor_after(1), VENDOR);
    assert_eq!(
        frontier(&created),
        [
            "n_decision_meeting",
            "n_kickoff",
            "n_meeting_date",
            "n_partner_runs",
            "n_purpose",
            "n_who_informed",
            "n_who_owns"
        ]
    );
    let setup = blocker(
        "n_kickoff",
        DependencyVia::StageOpening {
            group: key("n_setup"),
        },
    );
    assert!(blockers(&created, "n_setup").contains(&setup));
    assert!(
        blockers(&created, "n_access").is_empty(),
        "inherited, not copied"
    );
    assert_eq!(
        created.blocked_through(&graph, &key("n_access")),
        [key("n_setup")]
    );
    assert_eq!(
        blockers(&created, "n_comparison_set"),
        BTreeSet::from([blocker("n_plan", DependencyVia::Explicit)])
    );
    assert_eq!(
        created.relevance().value(&key("n_baseline")),
        Relevance::Undecided
    );
    let condition = DependencyVia::Condition {
        condition_on: key("n_baseline"),
    };
    assert_eq!(
        blockers(&created, "n_baseline"),
        BTreeSet::from([blocker("n_comparison_set", condition)]),
        "an undecided node waits on the decision that decides it"
    );
    let decided = support::derived(&support::vendor_after(2), VENDOR);
    assert_eq!(frontier(&decided), ["n_decision_meeting", "n_kickoff"]);
    assert!(
        decided.blocking().blocked(&key("n_access")),
        "Setup waits on kickoff"
    );
    let kicked_off = support::derived(&support::vendor_after(3), VENDOR);
    assert_eq!(
        frontier(&kicked_off),
        ["n_access", "n_decision_meeting", "n_workload"]
    );
    assert!(
        kicked_off.blocking().needs_breakdown(&key("n_workload")),
        "B10"
    );
}

const UP: &str =
    "{key: n_up, id: up, kind: decision, title: Up, prompt: Which?, answer_type: text}";
const WORK: &str = "{key: n_work, id: work, kind: action, title: Work}";
const GATED: &str = "{key: n_gated, id: gated, kind: decision, title: Gated, prompt: Go?, answer_type: boolean, requires: [n_work]}";
const AFTER: &str = "{key: n_after, id: after, kind: action, title: After, requires: [n_gated]}";
const BRANCH: &str = "{key: n_branch, id: branch, kind: action, title: Branch, relevant_when: {equals: {decision: n_gated, value: true}}}";
const LATER: &str = "{key: n_later, id: later, kind: action, title: Later, requires: [n_branch]}";

/// Gating 1 to 3: a decision with no open dependency is actionable from the start; a gated
/// decision waits on what it requires; a node that requires a decision, or whose condition
/// reads it, waits on its answer. An undecided dependency blocks like an open one and stops
/// blocking when the decision makes it not relevant.
#[test]
fn each_decision_placement_yields_its_frontier() {
    let records = support::journey(&add(&[UP, WORK, GATED, AFTER, BRANCH, LATER]));
    let start = derived(&records);
    assert_eq!(frontier(&start), ["n_up", "n_work"]);
    assert_eq!(
        blockers(&start, "n_later"),
        BTreeSet::from([blocker("n_branch", DependencyVia::Explicit)]),
        "the undecided branch blocks like an open one"
    );
    let worked = then(&records, &[transition("n_work", "complete")]);
    assert_eq!(frontier(&derived(&worked)), ["n_gated", "n_up"]);
    let yes = then(&worked, &[answer("n_gated", "{boolean: true}")]);
    assert_eq!(frontier(&derived(&yes)), ["n_after", "n_branch", "n_up"]);
    assert!(derived(&yes).blocking().blocked(&key("n_later")));
    let no = then(&worked, &[answer("n_gated", "{boolean: false}")]);
    let no = derived(&no);
    assert_eq!(frontier(&no), ["n_after", "n_later", "n_up"]);
    assert!(
        !no.blocking().blocked(&key("n_branch")) && !no.blocking().actionable(&key("n_branch")),
        "a not-relevant node is neither blocked nor actionable"
    );
}

const PLAN: &str = "{key: n_plan, id: plan, kind: deliverable, title: Plan}";
const DRAFT: &str = "{key: n_draft, id: draft, parent: n_plan, kind: action, title: Draft}";
const REVIEW: &str = "{key: n_review, id: review, parent: n_plan, kind: action, title: Review}";
const USES_PLAN: &str = "{key: n_uses, id: uses, kind: action, title: Uses, requires: [n_plan]}";

/// Containment: the plan reaches the frontier only after its child actions, and an edge
/// into the plan waits for the plan itself, not only its children.
#[test]
fn a_parent_reaches_the_frontier_only_after_its_children() {
    let records = support::journey(&add(&[PLAN, DRAFT, REVIEW, USES_PLAN]));
    assert_eq!(frontier(&derived(&records)), ["n_draft", "n_review"]);
    let children = then(
        &records,
        &[
            transition("n_draft", "complete"),
            transition("n_review", "complete"),
        ],
    );
    let children_done = derived(&children);
    assert_eq!(frontier(&children_done), ["n_plan"]);
    assert_eq!(
        blockers(&children_done, "n_uses"),
        BTreeSet::from([blocker("n_plan", DependencyVia::Explicit)]),
        "an edge into a parent waits for the parent's own completion"
    );
    let plan = then(&children, &[transition("n_plan", "complete")]);
    assert_eq!(frontier(&derived(&plan)), ["n_uses"]);
}

/// Containment, F4: an empty stage still waits for its opening gate, and what requires the
/// stage waits with it.
#[test]
fn an_empty_group_waits_for_its_opening_gate() {
    let records = support::journey(&add(&[
        "{key: n_opens, id: opens, kind: milestone, title: Opens}",
        "{key: n_stage, id: stage, kind: group, title: Stage, opens_at: n_opens}",
        "{key: n_next, id: next, kind: action, title: Next, requires: [n_stage]}",
    ]));
    let waiting = derived(&records);
    assert!(waiting.blocking().blocked(&key("n_stage")));
    assert!(!waiting.blocking().satisfies(&key("n_stage")));
    assert_eq!(
        blockers(&waiting, "n_next"),
        BTreeSet::from([blocker("n_stage", DependencyVia::Explicit)])
    );
    let opened = derived(&then(&records, &[transition("n_opens", "reach")]));
    assert!(
        opened.blocking().satisfies(&key("n_stage")),
        "done for edge purposes"
    );
    assert_eq!(frontier(&opened), ["n_next"]);
}

/// D1a: a skipped container satisfies its dependents only once the work kept beneath it
/// does.
#[test]
fn kept_work_holds_a_skipped_containers_dependents() {
    let records = support::journey(&add(&[PLAN, DRAFT, REVIEW, USES_PLAN]));
    let skipped = then(
        &records,
        &["- op: apply_override\n  node: n_review\n  override: {keep: {reason: Still needed.}}\n- op: transition\n  node: n_plan\n  transition: {skip: {reason: Not needed.}}\n".to_owned()],
    );
    let pending = derived(&skipped);
    assert!(
        !pending.blocking().satisfies(&key("n_plan")),
        "kept work pending"
    );
    assert!(pending.blocking().blocked(&key("n_uses")));
    assert_eq!(
        frontier(&pending),
        ["n_review"],
        "the effectively skipped draft is off it"
    );
    let kept_done = derived(&then(&skipped, &[transition("n_review", "complete")]));
    assert!(kept_done.blocking().satisfies(&key("n_plan")));
    assert_eq!(frontier(&kept_done), ["n_uses"]);
}

fn snooze(node: &str, until: &str) -> String {
    format!("- op: snooze\n  node: {node}\n  until: {until}\n")
}

/// B6, D2: a date snooze holds while today is before its date; the node stays on the
/// frontier and leaves the acting frontier while it holds.
#[test]
fn a_date_snooze_holds_until_its_date() {
    let records = support::journey(&add(&[WORK]));
    let snoozed = then(&records, &[snooze("n_work", "{date: \"2026-10-08\"}")]);
    let held = derived_on(&snoozed, "2026-10-07").1;
    assert_eq!(
        held.blocking().snoozed(&key("n_work")),
        Some(&SnoozeTarget::Date(date("2026-10-08")))
    );
    assert_eq!(frontier(&held), ["n_work"]);
    assert_eq!(acting(&held), [] as [&str; 0]);
    let lifted = derived_on(&snoozed, "2026-10-08").1;
    assert_eq!(lifted.blocking().snoozed(&key("n_work")), None);
    assert_eq!(acting(&lifted), ["n_work"]);
}

/// B6: a node snooze holds while its target neither satisfies dependencies nor is not
/// relevant, so it lifts when the target completes, holds again when it reopens, and lifts
/// when the target leaves scope.
#[test]
fn a_node_snooze_lifts_and_holds_again() {
    let target = "{key: n_target, id: target, kind: action, title: Target, relevant_when: {equals: {decision: n_flag, value: true}}}";
    let flag =
        "{key: n_flag, id: flag, kind: decision, title: Flag, prompt: Flag?, answer_type: boolean}";
    let records = then(
        &support::journey(&add(&[WORK, flag, target])),
        &[
            answer("n_flag", "{boolean: true}"),
            snooze("n_work", "{node: n_target}"),
        ],
    );
    let holds = |records: &Records| {
        derived(records)
            .blocking()
            .snoozed(&key("n_work"))
            .is_some()
    };
    assert!(holds(&records), "the target is open");
    let done = then(&records, &[transition("n_target", "complete")]);
    assert!(!holds(&done), "the target satisfies dependencies");
    let reopened = then(&done, &[transition("n_target", "reopen")]);
    assert!(holds(&reopened), "it holds again when the target reopens");
    let out = then(&reopened, &[answer("n_flag", "{boolean: false}")]);
    assert!(!holds(&out), "the target is not relevant");
}

const GATE: &str =
    "{key: n_gate, id: gate, kind: milestone, title: Gate, auto_reach: true, requires: [n_work]}";
const OPENS: &str = "{key: n_opened, id: opened, kind: action, title: Opened, requires: [n_gate]}";

fn pin(node: &str, on: &str) -> String {
    format!("- op: set_pin\n  node: {node}\n  date: \"{on}\"\n")
}

/// F1: a pending `auto_reach` milestone reads as reached once its effective date is today or
/// earlier and its dependencies are satisfied: it flips with the date, and reopening a
/// dependency makes it pending again. Until its date it stays on the frontier but off the
/// acting frontier.
#[test]
fn auto_reach_flips_with_the_date_and_reverses_with_a_dependency() {
    let records = then(
        &support::journey(&add(&[WORK, GATE, OPENS])),
        &[
            transition("n_work", "complete"),
            pin("n_gate", "2026-10-08"),
        ],
    );
    let ahead = derived_on(&records, "2026-10-07").1;
    assert!(!ahead.blocking().auto_reached(&key("n_gate")));
    assert_eq!(frontier(&ahead), ["n_gate"]);
    assert_eq!(acting(&ahead), [] as [&str; 0], "its date is still ahead");
    let arrived = derived_on(&records, "2026-10-08").1;
    assert!(arrived.blocking().auto_reached(&key("n_gate")));
    assert_eq!(frontier(&arrived), ["n_opened"]);
    let reopened = then(&records, &[transition("n_work", "reopen")]);
    let reverted = derived_on(&reopened, "2026-10-08").1;
    assert!(!reverted.blocking().auto_reached(&key("n_gate")));
    assert!(reverted.blocking().blocked(&key("n_opened")));
}

/// F1: auto-reach chains through two milestones, and a pending `auto_reach` milestone pinned
/// yesterday with its dependencies satisfied reads as reached, while a plain pinned milestone
/// still needs a manual reach.
#[test]
fn auto_reach_chains_through_two_milestones() {
    let records = then(
        &support::journey(&add(&[
            "{key: n_first, id: first, kind: milestone, title: First, auto_reach: true}",
            "{key: n_second, id: second, kind: milestone, title: Second, auto_reach: true, requires: [n_first]}",
            "{key: n_plain, id: plain, kind: milestone, title: Plain, requires: [n_second]}",
            "{key: n_next, id: next, kind: action, title: Next, requires: [n_plain]}",
        ])),
        &[
            pin("n_first", "2026-10-01"),
            pin("n_second", "2026-10-05"),
            pin("n_plain", "2026-10-05"),
        ],
    );
    let reached = derived(&records);
    for milestone in ["n_first", "n_second"] {
        assert!(
            reached.blocking().auto_reached(&key(milestone)),
            "{milestone}"
        );
    }
    assert!(!reached.blocking().auto_reached(&key("n_plain")));
    // Review round 1: a milestone that reads as reached is not overdue (F1, F6).
    assert!(!reached.dates().overdue(&key("n_first")));
    assert!(
        reached.dates().overdue(&key("n_plain")),
        "a plain pin past is still open"
    );
    assert_eq!(
        frontier(&reached),
        ["n_plain"],
        "a plain pin is reached by hand"
    );
}

/// D5: an empty acting frontier with open work is stalled, and the diagnostic names each
/// snooze with its target, each `auto_reach` date still ahead, and the gating node a node
/// snooze waits on.
#[test]
fn stalled_names_a_gate_a_snooze_and_an_auto_reach_date() {
    let records = then(
        &support::journey(&add(&[
            "{key: n_kickoff, id: kickoff, kind: milestone, title: Kickoff, auto_reach: true}",
            "{key: n_access, id: access, kind: action, title: Access, requires: [n_kickoff]}",
            "{key: n_read, id: read, kind: action, title: Read}",
            "{key: n_plan, id: plan, kind: action, title: Plan}",
        ])),
        &[
            pin("n_kickoff", "2026-10-10"),
            snooze("n_read", "{date: \"2026-10-09\"}"),
            snooze("n_plan", "{node: n_access}"),
        ],
    );
    let stalled = derived(&records);
    assert_eq!(acting(&stalled), [] as [&str; 0]);
    let found = stalled.blocking().stalled().expect("stalled (D5)");
    // The held nodes in key order, then the gating nodes.
    assert_eq!(
        found.waiting_on,
        [
            StallCause::AutoReach {
                node: key("n_kickoff"),
                date: date("2026-10-10"),
            },
            StallCause::Snooze {
                node: key("n_plan"),
                until: SnoozeTarget::Node(key("n_access")),
            },
            StallCause::Snooze {
                node: key("n_read"),
                until: SnoozeTarget::Date(date("2026-10-09")),
            },
            StallCause::Gate(key("n_access")),
        ]
    );
    assert!(!found.all_blocked, "the held nodes are on the frontier");
    let lifted = derived_on(&records, "2026-10-09").1;
    assert_eq!(acting(&lifted), ["n_read"]);
    assert!(lifted.blocking().stalled().is_none());
}

const DONE_WORK: &str = "{key: n_done, id: done, kind: action, title: Done}";

/// D4: a completion goes stale when a dependency is inserted after it, and is listed with the
/// open dependency; it is not blocked, and never for what its own skip did not require.
#[test]
fn an_inserted_dependency_makes_a_completion_stale() {
    let records = support::journey(&add(&[
        DONE_WORK,
        "{key: n_skipped, id: skipped, kind: action, title: Skipped}",
    ]));
    let finished = then(
        &records,
        &[
            transition("n_done", "complete"),
            transition("n_skipped", "{skip: {reason: Not needed.}}"),
        ],
    );
    let inserted = then(
        &finished,
        &[format!(
            "{}- op: add_edge\n  edge: {{node: n_done, requires: n_new}}\n- op: add_edge\n  edge: {{node: n_skipped, requires: n_new}}\n",
            add(&["{key: n_new, id: new, kind: action, title: New}"])
        )],
    );
    let (graph, derived) = derived_on(&inserted, "2026-10-06");
    assert_eq!(
        derived.stale(&graph, &key("n_done")),
        BTreeSet::from([GuardFailure::OpenDependency(key("n_new"))])
    );
    assert!(
        !derived.blocking().blocked(&key("n_done")),
        "visible, not blocking"
    );
    assert!(
        !derived.is_stale(&key("n_skipped")),
        "a skip required nothing"
    );
}

/// D4: an answer changed after a completion makes a node relevant that it now waits on, so
/// the completion goes stale.
#[test]
fn an_answer_change_makes_a_completion_stale() {
    let flag =
        "{key: n_flag, id: flag, kind: decision, title: Flag, prompt: Flag?, answer_type: boolean}";
    let branch = "{key: n_branch, id: branch, kind: action, title: Branch, relevant_when: {equals: {decision: n_flag, value: true}}}";
    let user = "{key: n_user, id: user, kind: action, title: User, requires: [n_branch]}";
    let records = then(
        &support::journey(&add(&[flag, branch, user])),
        &[
            answer("n_flag", "{boolean: false}"),
            transition("n_user", "complete"),
        ],
    );
    assert!(!derived(&records).is_stale(&key("n_user")));
    let revised = then(&records, &[answer("n_flag", "{boolean: true}")]);
    let (graph, revised) = derived_on(&revised, "2026-10-06");
    assert_eq!(
        revised.stale(&graph, &key("n_user")),
        BTreeSet::from([GuardFailure::OpenDependency(key("n_branch"))])
    );
}

/// B10, D4: a placeholder needs breakdown until it has children or is marked atomic.
#[test]
fn a_placeholder_needs_breakdown_until_it_has_children_or_is_atomic() {
    let records = support::journey(&add(&[
        "{key: n_holder, id: holder, kind: deliverable, title: Holder, placeholder: true}",
    ]));
    let needs = |records: &Records| {
        derived(records)
            .blocking()
            .needs_breakdown(&key("n_holder"))
    };
    assert!(needs(&records));
    let atomic = then(
        &records,
        &["- op: set_atomic\n  node: n_holder\n  atomic: true\n".to_owned()],
    );
    assert!(!needs(&atomic));
    let broken = then(
        &records,
        &[add(&[
            "{key: n_part, id: part, parent: n_holder, kind: action, title: Part}",
        ])],
    );
    assert!(!needs(&broken));
}

/// Review round 1: a skipped container whose own requirement is open still lists it, so the
/// kept work blocked through it can follow the blocker to its source (PRD Containment).
#[test]
fn a_closed_ancestor_lists_what_it_holds_back() {
    let records = support::journey(&add(&[
        WORK,
        "{key: n_box, id: box, kind: deliverable, title: Box, requires: [n_work]}",
        "{key: n_inside, id: inside, parent: n_box, kind: action, title: Inside}",
    ]));
    let skipped = then(
        &records,
        &["- op: apply_override\n  node: n_inside\n  override: {keep: {reason: Still needed.}}\n- op: transition\n  node: n_box\n  transition: {skip: {reason: Not needed.}}\n".to_owned()],
    );
    let (graph, derived) = derived_on(&skipped, "2026-10-06");
    assert!(derived.blocking().blocked(&key("n_inside")));
    assert_eq!(
        derived.blocked_through(&graph, &key("n_inside")),
        [key("n_box")]
    );
    assert!(!derived.blocking().blocked(&key("n_box")), "closed");
    assert_eq!(
        blockers(&derived, "n_box"),
        BTreeSet::from([blocker("n_work", DependencyVia::Explicit)])
    );
}
