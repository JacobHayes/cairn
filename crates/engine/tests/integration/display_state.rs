//! D8, display state: the one state every surface shows for a node. A scenario per row of the
//! precedence table over hand-built journeys, the vendor evaluation, the hiring loop and the
//! product launch; the pending rule on a decision gated on another decision (Gating, Kleene
//! chains); the counts beside `by_state`, which stays what it was; and the narrowed
//! `unassigned` (E1). Derived at 2026-10-06 unless a test says otherwise.
#![cfg(test)]

use crate::support;

use std::collections::{BTreeMap, BTreeSet};

use DisplayState::{
    Active, Blocked, Conditional, Done, NotRelevant, Ready, Scheduled, Skipped, Snoozed,
};
use cairn_engine::{Derived, DerivedJourney, Graph, Records};
use cairn_schema::{
    DisplayState, NextQuery, NodeKind, Relevance, SnapshotScope, State, StatusSummary,
};
use support::{add_nodes as add, key};

/// A journey's graph and its derive at `today`.
struct Seen {
    graph: Graph,
    derived: Derived,
}

impl Seen {
    fn of(records: &Records, journey: &str, today: &str) -> Self {
        Self {
            graph: support::journey_graph(records, journey),
            derived: support::derived_on(records, journey, today.parse().unwrap()),
        }
    }

    fn test(records: &Records) -> Self {
        Self::of(records, support::JOURNEY, "2026-10-06")
    }

    fn shown(&self, node: &str) -> DisplayState {
        self.derived.display_state(&self.graph, &key(node))
    }

    fn stored(&self, node: &str) -> State {
        let node = key(node);
        let kind = self.graph.node(&node).unwrap().kind();
        let stored = self.graph.document().state.nodes.get(&node);
        stored.map_or(State::initial(kind), |stored| stored.state)
    }

    fn relevance(&self, node: &str) -> Relevance {
        self.derived.relevance().value(&key(node))
    }

    fn journey(&self) -> DerivedJourney<'_> {
        DerivedJourney::new(&self.graph, &self.derived)
    }
}

fn transition(node: &str, what: &str) -> String {
    format!("- op: transition\n  node: {node}\n  transition: {what}\n")
}

fn skip(node: &str) -> String {
    format!("- op: transition\n  node: {node}\n  transition: {{skip: {{reason: Not needed.}}}}\n")
}

fn answer(decision: &str, value: &str) -> String {
    format!("- op: answer\n  decision: {decision}\n  value: {value}\n")
}

fn snooze(node: &str, until: &str) -> String {
    format!("- op: snooze\n  node: {node}\n  until: {until}\n")
}

fn pin(node: &str, on: &str) -> String {
    format!("- op: set_pin\n  node: {node}\n  date: \"{on}\"\n")
}

fn then(records: &Records, steps: &[String]) -> Records {
    support::accepted(records, &steps.concat())
}

const ASK: &str =
    "{key: n_ask, id: ask, kind: decision, title: Ask, prompt: Ask?, answer_type: boolean}";
const BRANCH: &str = "{key: n_branch, id: branch, kind: decision, title: Branch, prompt: Branch?, answer_type: boolean, relevant_when: {equals: {decision: n_ask, value: true}}}";
const INNER: &str = "{key: n_inner, id: inner, kind: action, title: Inner, relevant_when: {equals: {decision: n_branch, value: true}}}";
const PLAIN: &str = "{key: n_plain, id: plain, kind: action, title: Plain}";
const FIRST: &str = "{key: n_first, id: first, kind: action, title: First}";
const SECOND: &str =
    "{key: n_second, id: second, kind: action, title: Second, requires: [n_first]}";
const GATE: &str = "{key: n_gate, id: gate, kind: milestone, title: Gate, auto_reach: true}";
const STAGE: &str = "{key: n_stage, id: stage, kind: deliverable, title: Stage}";
const STEP_A: &str = "{key: n_step_a, id: step-a, parent: n_stage, kind: action, title: Step A}";
const STEP_B: &str = "{key: n_step_b, id: step-b, parent: n_stage, kind: action, title: Step B}";
const WALLED: &str =
    "{key: n_walled, id: walled, kind: deliverable, title: Walled, requires: [n_first]}";
const WALL_KID: &str =
    "{key: n_wall_kid, id: wall-kid, parent: n_walled, kind: action, title: Wall kid}";
const BOX: &str = "{key: n_box, id: box, kind: group, title: Box}";
const BOX_KID: &str = "{key: n_box_kid, id: box-kid, parent: n_box, kind: action, title: Box kid}";
const NAP: &str = "{key: n_nap, id: nap, kind: action, title: Nap}";
const DROP: &str = "{key: n_drop, id: drop, kind: action, title: Drop}";
const FINISH: &str = "{key: n_finish, id: finish, kind: action, title: Finish}";
const BEGUN: &str = "{key: n_begun, id: begun, kind: action, title: Begun}";

fn board() -> Records {
    support::journey(&add(&[
        ASK, BRANCH, INNER, PLAIN, FIRST, SECOND, GATE, STAGE, STEP_A, STEP_B, WALLED, WALL_KID,
        BOX, BOX_KID, NAP, DROP, FINISH, BEGUN,
    ]))
}

/// D8 row 9, ready: actionable work, and a container waiting only on its own children with
/// nothing started, which is never blocked by them.
#[test]
fn ready_is_actionable_work_and_a_container_waiting_on_its_children() {
    let seen = Seen::test(&board());
    assert_eq!(seen.shown("n_plain"), Ready);
    assert_eq!(seen.shown("n_first"), Ready);
    assert_eq!(seen.shown("n_stage"), Ready, "waits only on its two steps");
    assert!(
        !seen.derived.blocking().actionable(&key("n_stage")),
        "technically not actionable: it waits on its children"
    );
    assert_eq!(seen.shown("n_box"), Ready, "a group not started");
    let finished = then(
        &board(),
        &[
            transition("n_step_a", "complete"),
            transition("n_step_b", "complete"),
        ],
    );
    let finished = Seen::test(&finished);
    assert_eq!(finished.shown("n_stage"), Ready, "ready to finish");
    assert_eq!(finished.stored("n_stage"), State::Todo);
}

/// D8 row 5, active: stored active, and a container whose only unsatisfied waits are its own
/// children once some work beneath it has started, not blocked.
#[test]
fn active_is_started_work_and_a_container_with_work_beneath_it_started() {
    let started = then(
        &board(),
        &[
            transition("n_begun", "start"),
            transition("n_step_a", "start"),
        ],
    );
    let seen = Seen::test(&started);
    assert_eq!(seen.shown("n_begun"), Active);
    assert_eq!(seen.shown("n_stage"), Active);
    assert!(
        seen.derived.blocking().blocked(&key("n_stage")),
        "the flag stays"
    );
    assert_eq!(
        seen.stored("n_stage"),
        State::Todo,
        "stored state is untouched"
    );
    let done_beneath = then(&started, &[transition("n_step_a", "complete")]);
    assert_eq!(Seen::test(&done_beneath).shown("n_stage"), Active);
    let group = then(&board(), &[transition("n_box_kid", "start")]);
    assert_eq!(Seen::test(&group).shown("n_box"), Active);
}

/// D8 row 7, blocked: an unsatisfied own gate; a container is blocked only by its own
/// gates, even when work beneath it has started.
#[test]
fn blocked_is_an_unsatisfied_gate_never_a_containers_children() {
    let seen = Seen::test(&board());
    assert_eq!(seen.shown("n_second"), Blocked);
    assert_eq!(seen.shown("n_walled"), Blocked, "waits on n_first");
    assert_eq!(
        seen.shown("n_wall_kid"),
        Blocked,
        "inherited from the container"
    );
    let done = then(&board(), &[transition("n_first", "complete")]);
    let seen = Seen::test(&done);
    assert_eq!(seen.shown("n_second"), Ready);
    assert_eq!(seen.shown("n_walled"), Ready);
    let started = then(&board(), &[transition("n_wall_kid", "start")]);
    assert_eq!(
        Seen::test(&started).shown("n_walled"),
        Blocked,
        "its own gate holds, whatever has started beneath it"
    );
}

/// D8 row 3, done: stored done, decided and reached, an auto-reached milestone and a group
/// that satisfies dependencies.
#[test]
fn done_covers_finished_work_decisions_milestones_and_groups() {
    let records = then(
        &board(),
        &[
            transition("n_finish", "complete"),
            answer("n_ask", "{boolean: true}"),
            transition("n_box_kid", "complete"),
        ],
    );
    let seen = Seen::test(&records);
    assert_eq!(seen.shown("n_finish"), Done);
    assert_eq!(seen.shown("n_ask"), Done);
    assert_eq!(seen.stored("n_ask"), State::Decided);
    assert_eq!(
        seen.shown("n_box"),
        Done,
        "a group that satisfies dependencies"
    );
    let reached = then(&records, &[transition("n_gate", "reach")]);
    assert_eq!(Seen::test(&reached).shown("n_gate"), Done);
    let automatic = then(&records, &[pin("n_gate", "2026-10-05")]);
    let automatic = Seen::test(&automatic);
    assert!(automatic.derived.blocking().auto_reached(&key("n_gate")));
    assert_eq!(automatic.shown("n_gate"), Done, "auto-reached");
    assert_eq!(automatic.stored("n_gate"), State::Pending);
}

/// D8 row 2, skipped: stored, and effective through a skipped container.
#[test]
fn skipped_is_stored_or_effective() {
    let records = then(&board(), &[skip("n_drop"), skip("n_stage")]);
    let seen = Seen::test(&records);
    assert_eq!(seen.shown("n_drop"), Skipped);
    assert_eq!(seen.shown("n_stage"), Skipped);
    assert_eq!(seen.shown("n_step_a"), Skipped, "with its container");
    assert_eq!(seen.stored("n_step_a"), State::Todo);
    assert!(seen.derived.skips().skipped_by(&key("n_step_a")).is_some());
}

/// D8 row 4, snoozed: a snooze that holds, and not once it lapses.
#[test]
fn snoozed_while_the_snooze_holds() {
    let records = then(&board(), &[snooze("n_nap", "{date: \"2026-10-09\"}")]);
    assert_eq!(Seen::test(&records).shown("n_nap"), Snoozed);
    let lapsed = Seen::of(&records, support::JOURNEY, "2026-10-09");
    assert_eq!(lapsed.shown("n_nap"), Ready);
    let held = then(
        &board(),
        &[
            transition("n_step_a", "start"),
            snooze("n_stage", "{date: \"2026-10-09\"}"),
        ],
    );
    let held = Seen::test(&held);
    assert_eq!(
        held.shown("n_stage"),
        Snoozed,
        "snoozed beats the work started beneath it: the person set it aside"
    );
    assert_eq!(held.shown("n_step_a"), Active);
}

/// D8 row 8, scheduled: an unblocked `auto_reach` milestone whose date is ahead; ready
/// without a date, blocked behind its work.
#[test]
fn scheduled_is_an_auto_reach_milestone_whose_date_is_ahead() {
    let dated = then(&board(), &[pin("n_gate", "2026-10-20")]);
    let seen = Seen::test(&dated);
    assert_eq!(seen.shown("n_gate"), Scheduled);
    assert_eq!(seen.stored("n_gate"), State::Pending);
    assert_eq!(
        Seen::of(&dated, support::JOURNEY, "2026-10-20").shown("n_gate"),
        Done
    );
    assert_eq!(Seen::test(&board()).shown("n_gate"), Ready, "no date");
    let gated = support::journey(&add(&[
        FIRST,
        "{key: n_gate, id: gate, kind: milestone, title: Gate, auto_reach: true, requires: [n_first]}",
    ]));
    let gated = then(&gated, &[pin("n_gate", "2026-10-20")]);
    assert_eq!(Seen::test(&gated).shown("n_gate"), Blocked);
}

/// D8 row 6, conditional: relevance undecided and unfinished; finished while undecided it is
/// done (D4).
#[test]
fn conditional_is_undecided_relevance_until_finished() {
    let seen = Seen::test(&board());
    assert_eq!(seen.relevance("n_branch"), Relevance::Undecided);
    assert_eq!(seen.shown("n_branch"), Conditional);
    let finished = then(
        &support::journey(&add(&[
            ASK,
            "{key: n_late, id: late, kind: action, title: Late, relevant_when: {equals: {decision: n_ask, value: true}}}",
        ])),
        &[transition("n_late", "complete")],
    );
    let seen = Seen::test(&finished);
    assert_eq!(seen.relevance("n_late"), Relevance::Undecided);
    assert_eq!(seen.shown("n_late"), Done, "finished while undecided");
}

/// D8 row 1: the user's case. A decision gated out by an answer is not relevant, whatever
/// its stored state, which survives as the recorded state: not OPEN.
#[test]
fn a_decision_gated_out_by_an_answer_is_not_relevant_not_open() {
    let records = then(&board(), &[answer("n_ask", "{boolean: false}")]);
    let seen = Seen::test(&records);
    assert_eq!(seen.relevance("n_branch"), Relevance::NotRelevant);
    assert_eq!(seen.stored("n_branch"), State::Open, "recorded Open");
    assert_eq!(seen.shown("n_branch"), NotRelevant);
    assert_eq!(
        seen.shown("n_inner"),
        NotRelevant,
        "settled: nothing can change it"
    );
    assert!(
        seen.derived
            .relevance()
            .get(&key("n_inner"))
            .unwrap()
            .pending_on
            .is_empty()
    );
}

/// D8, not relevant beats done and skipped: finished work that stops applying shows as not
/// relevant, with its recorded state kept (B2).
#[test]
fn not_relevant_beats_done_and_skipped() {
    let late = "{key: n_late, id: late, kind: action, title: Late, relevant_when: {equals: {decision: n_ask, value: true}}}";
    let undecided = support::journey(&add(&[ASK, late]));
    let records = then(
        &then(&undecided, &[transition("n_late", "complete")]),
        &[answer("n_ask", "{boolean: false}")],
    );
    let seen = Seen::test(&records);
    assert_eq!(seen.stored("n_late"), State::Done, "recorded Done");
    assert_eq!(seen.shown("n_late"), NotRelevant);
    let skipped = then(
        &then(&undecided, &[skip("n_late")]),
        &[answer("n_ask", "{boolean: false}")],
    );
    assert_eq!(Seen::test(&skipped).shown("n_late"), NotRelevant);
}

/// D8's pending rule over a decision gated on another decision (Gating: a decision that is
/// itself undecided is unanswered). Before the first answer the second-order branch is
/// conditional, pending on the inner decision, though its relevance is still not relevant;
/// each answer settles it.
#[test]
fn a_branch_behind_an_undecided_decision_is_conditional_not_ruled_out() {
    let before = Seen::test(&board());
    assert_eq!(before.relevance("n_branch"), Relevance::Undecided);
    assert_eq!(before.relevance("n_inner"), Relevance::NotRelevant);
    assert_eq!(before.shown("n_inner"), Conditional);
    let found = before.derived.relevance().get(&key("n_inner")).unwrap();
    assert_eq!(
        found.pending_on.iter().collect::<Vec<_>>(),
        [&key("n_branch")],
        "names the inner decision"
    );
    let schema = before.derived.node_derived(&before.graph, &key("n_inner"));
    assert_eq!(schema.relevance.value, Relevance::NotRelevant);
    assert_eq!(schema.relevance.pending_on, found.pending_on);
    assert_eq!(schema.display_state, Conditional);
    assert!(
        !before.derived.blocking().blocked(&key("n_inner")),
        "blocks nothing"
    );
    assert_eq!(
        before
            .derived
            .priority()
            .gravity(&key("n_inner"))
            .millionths(),
        0,
        "not counted in priority until then"
    );

    let outer_yes = Seen::test(&then(&board(), &[answer("n_ask", "{boolean: true}")]));
    assert_eq!(outer_yes.relevance("n_inner"), Relevance::Undecided);
    assert_eq!(outer_yes.shown("n_inner"), Conditional);
    assert!(
        outer_yes
            .derived
            .relevance()
            .get(&key("n_inner"))
            .unwrap()
            .pending_on
            .is_empty()
    );

    let outer_no = Seen::test(&then(&board(), &[answer("n_ask", "{boolean: false}")]));
    assert_eq!(outer_no.shown("n_inner"), NotRelevant, "ruled out for good");

    let both_yes = then(
        &board(),
        &[
            answer("n_ask", "{boolean: true}"),
            answer("n_branch", "{boolean: true}"),
        ],
    );
    let both_yes = Seen::test(&both_yes);
    assert_eq!(both_yes.shown("n_inner"), Ready);
    let inner_no = then(
        &board(),
        &[
            answer("n_ask", "{boolean: true}"),
            answer("n_branch", "{boolean: false}"),
        ],
    );
    assert_eq!(Seen::test(&inner_no).shown("n_inner"), NotRelevant);
}

/// D8: pending carries through a chain and through a container, a settled reason beside it
/// keeps the node not relevant, and a skip above an undecided decision that was already
/// decided does not settle what reads it (D1a reaches only what is still open).
#[test]
fn pending_follows_chains_and_containers_and_yields_to_a_settled_reason() {
    let outer = "{key: n_outer, id: outer, kind: group, title: Outer, relevant_when: {equals: {decision: n_branch, value: true}}}";
    let nested = "{key: n_nested, id: nested, parent: n_outer, kind: action, title: Nested}";
    let both = "{key: n_both, id: both, kind: action, title: Both, relevant_when: {all: [{equals: {decision: n_branch, value: true}}, {equals: {decision: n_other, value: true}}]}}";
    let other = "{key: n_other, id: other, kind: decision, title: Other, prompt: Other?, answer_type: boolean}";
    let records = support::journey(&add(&[ASK, BRANCH, outer, nested, other, both]));
    let seen = Seen::test(&records);
    assert_eq!(seen.shown("n_outer"), Conditional);
    assert_eq!(
        seen.shown("n_nested"),
        Conditional,
        "under a pending container"
    );
    let pending = seen.derived.relevance().get(&key("n_nested")).unwrap();
    assert_eq!(
        pending.pending_on.iter().collect::<Vec<_>>(),
        [&key("n_branch")]
    );
    assert_eq!(
        seen.relevance("n_both"),
        Relevance::NotRelevant,
        "n_branch is undecided, so unanswered"
    );
    assert_eq!(seen.shown("n_both"), Conditional);
    let settled = then(&records, &[answer("n_other", "{boolean: false}")]);
    let settled = Seen::test(&settled);
    assert_eq!(
        settled.shown("n_both"),
        NotRelevant,
        "the other answer rules it out"
    );
    assert!(
        settled
            .derived
            .relevance()
            .get(&key("n_both"))
            .unwrap()
            .pending_on
            .is_empty()
    );
    assert_eq!(settled.shown("n_nested"), Conditional);

    let held = "{key: n_held, id: held, parent: n_box, kind: decision, title: Held, prompt: Held?, answer_type: boolean, relevant_when: {equals: {decision: n_ask, value: true}}}";
    let reader = "{key: n_reader, id: reader, kind: action, title: Reader, relevant_when: {equals: {decision: n_held, value: true}}}";
    let decided = then(
        &support::journey(&add(&[ASK, BOX, held, reader])),
        &[answer("n_held", "{boolean: true}")],
    );
    let skipped = Seen::test(&then(&decided, &[skip("n_box")]));
    assert_eq!(skipped.relevance("n_reader"), Relevance::NotRelevant);
    assert_eq!(
        skipped.shown("n_reader"),
        Conditional,
        "its answer returns when it applies"
    );
}

/// D8 on the vendor evaluation: answering "no" to the partner decision rules out the
/// partner-led subtree, a decision added beneath it included (recorded Open); Setup's
/// plan, a deliverable with active children, is active rather than blocked.
#[test]
fn the_vendor_evaluation_after_its_decisions() {
    let added = support::accepted_on(
        &support::vendor_after(2),
        "j_vendor_eval",
        &add(&[
            "{key: n_partner_scope, id: scope, parent: n_partner_led, kind: decision, title: Partner scope, prompt: How far does the partner go?, answer_type: boolean}",
        ]),
    );
    let seen = Seen::of(&added, "j_vendor_eval", "2026-10-06");
    assert_eq!(seen.relevance("n_partner_scope"), Relevance::NotRelevant);
    assert_eq!(seen.stored("n_partner_scope"), State::Open);
    assert_eq!(seen.shown("n_partner_scope"), NotRelevant);
    assert_eq!(seen.shown("n_partner_led"), NotRelevant);
    assert_eq!(seen.shown("n_criteria"), NotRelevant);
    assert_eq!(seen.shown("n_who_owns"), Done);
    assert_eq!(seen.shown("n_kickoff"), Ready);
    assert_eq!(
        seen.shown("n_setup"),
        Blocked,
        "waits for the kickoff that opens it"
    );
    assert_eq!(
        seen.shown("n_baseline"),
        Conditional,
        "undecided until the comparison set"
    );

    let before = Seen::of(&support::vendor_after(1), "j_vendor_eval", "2026-10-06");
    assert_eq!(before.shown("n_partner_led"), Conditional);
    assert_eq!(before.shown("n_partner_runs"), Ready);

    let working = Seen::of(&support::vendor_after(5), "j_vendor_eval", "2026-10-08");
    assert_eq!(working.stored("n_plan"), State::Todo);
    assert_eq!(
        working.shown("n_plan"),
        Ready,
        "its children wait, nothing started"
    );
    let started = then_on(
        &support::vendor_after(5),
        "j_vendor_eval",
        &[transition("n_plan_draft", "start")],
    );
    let started = Seen::of(&started, "j_vendor_eval", "2026-10-08");
    assert_eq!(started.shown("n_plan"), Active);
    assert_eq!(started.stored("n_plan"), State::Todo);
    assert_eq!(started.shown("n_setup"), Active);
    assert_eq!(started.shown("n_workload"), Ready);
}

fn then_on(records: &Records, journey: &str, steps: &[String]) -> Records {
    support::accepted_on(records, journey, &steps.concat())
}

/// D8 on the hiring loop: a skipped screen, the loop active, the offer ruled out by the
/// decision and the close-out relevant once the decision is made.
#[test]
fn the_hiring_loop_through_its_scenario() {
    let screened = Seen::of(&support::after("hiring-loop", 3), "j_hiring", "2026-10-06");
    assert_eq!(screened.shown("n_screen"), Skipped);
    assert_eq!(
        screened.shown("n_offer"),
        Conditional,
        "make-offer is still open"
    );
    assert_eq!(screened.shown("n_close_out"), Conditional);
    assert_eq!(screened.shown("n_choose_panel"), Done);
    let all = support::finished("hiring-loop");
    let last = Seen::of(&all, "j_hiring", "2026-10-30");
    assert_eq!(last.shown("n_make_offer"), Done);
    assert_eq!(last.shown("n_close_out"), NotRelevant, "an offer was made");
    assert_eq!(last.shown("n_screen"), Skipped);
}

/// D8 on the product launch: an auto-reach milestone is scheduled while its date is ahead,
/// and done once the date arrives, its stored state still pending.
#[test]
fn the_product_launch_schedules_its_auto_reach_milestone() {
    let records = support::after("product-launch", 3);
    let ahead = Seen::of(&records, "j_launch", "2026-09-06");
    assert_eq!(ahead.shown("n_beta_start"), Scheduled);
    assert_eq!(ahead.shown("n_features"), Active);
    assert_eq!(
        ahead.shown("n_build"),
        Active,
        "a group with started work beneath it"
    );
    assert_eq!(ahead.shown("n_hardening"), Blocked);
    let arrived = Seen::of(&records, "j_launch", "2027-06-01");
    assert_eq!(arrived.shown("n_beta_start"), Done);
    assert_eq!(arrived.stored("n_beta_start"), State::Pending);
}

/// D8: the count by display state agrees with the status summary, the snapshot and the next
/// list on every fixture after every step; `by_state` is what it was: the in-scope nodes by
/// stored state.
#[test]
fn counts_agree_everywhere_and_by_state_is_unchanged() {
    for name in support::fixture_names() {
        let scenario = support::scenario(&name);
        let steps = scenario.steps.as_slice().len();
        let journey = scenario.journey.to_string();
        for step in 1..=steps {
            let records = support::after(&name, step);
            let seen = Seen::of(&records, &journey, "2026-12-01");
            let summary: StatusSummary = seen.journey().status_summary();
            let snapshot = seen
                .journey()
                .snapshot(&SnapshotScope::default())
                .unwrap()
                .counts;
            let mut stored: BTreeMap<State, u32> = BTreeMap::new();
            let mut shown: BTreeMap<DisplayState, u32> = BTreeMap::new();
            for node in seen.graph.document().nodes.as_map().keys() {
                if seen.derived.relevance().in_scope(node) {
                    let state = seen.stored(node.as_str());
                    *stored.entry(state).or_default() += 1;
                    *shown
                        .entry(seen.derived.display_state(&seen.graph, node))
                        .or_default() += 1;
                }
            }
            assert_eq!(summary.by_state, stored, "{name} step {step}");
            assert_eq!(snapshot.by_state, stored, "{name} step {step}");
            assert_eq!(summary.by_display_state, shown, "{name} step {step}");
            assert_eq!(snapshot.by_display_state, shown, "{name} step {step}");
            assert!(!shown.contains_key(&NotRelevant), "{name} step {step}");
            let next = seen
                .journey()
                .next(&NextQuery::default(), &BTreeSet::default())
                .unwrap();
            for row in &next.items {
                assert!(
                    matches!(row.display_state, Ready | Active),
                    "{name} step {step}: {} is {:?}",
                    row.key,
                    row.display_state
                );
                assert_eq!(
                    row.display_state,
                    seen.derived.display_state(&seen.graph, &row.key)
                );
            }
        }
    }
}

/// D8: the level carries it on every node, groups' legacy `group_state` included, and the
/// legacy value still says what it did.
#[test]
fn the_level_carries_display_state_and_keeps_group_state() {
    let started = then(&board(), &[transition("n_box_kid", "start")]);
    let seen = Seen::test(&started);
    let all = [
        NodeKind::Group,
        NodeKind::Action,
        NodeKind::Deliverable,
        NodeKind::Decision,
        NodeKind::Milestone,
    ];
    let level = seen
        .journey()
        .level(&all.into_iter().collect(), None)
        .unwrap();
    for node in &level.nodes {
        assert_eq!(
            node.display_state,
            seen.derived.display_state(&seen.graph, &node.key)
        );
    }
    let boxed = level
        .nodes
        .iter()
        .find(|node| node.key == key("n_box"))
        .unwrap();
    assert_eq!(boxed.display_state, Active);
    assert_eq!(boxed.group_state, Some(cairn_schema::GroupState::Active));
    let plain = level
        .nodes
        .iter()
        .find(|node| node.key == key("n_plain"))
        .unwrap();
    assert_eq!(plain.group_state, None);
}

/// E1: `unassigned` is for non-group, in-scope, unfinished nodes only: not a group, not a
/// finished node, not a not-relevant one.
#[test]
fn unassigned_is_for_unfinished_in_scope_work() {
    let records = then(
        &board(),
        &[
            transition("n_finish", "complete"),
            answer("n_ask", "{boolean: false}"),
        ],
    );
    let seen = Seen::test(&records);
    let unassigned = |node: &str| seen.derived.is_unassigned(&seen.graph, &key(node));
    assert!(
        seen.derived.participation().is_unassigned(&key("n_finish")),
        "no owner"
    );
    assert!(unassigned("n_plain"));
    assert!(!unassigned("n_box"), "a group");
    assert!(!unassigned("n_finish"), "finished");
    assert!(!unassigned("n_branch"), "not relevant");
    assert!(!unassigned("n_inner"), "not relevant");
    let schema = seen.derived.node_derived(&seen.graph, &key("n_finish"));
    assert!(!schema.unassigned);
    let row = seen.journey().snapshot(&SnapshotScope::default()).unwrap();
    assert!(row.unassigned.contains(&key("n_plain")));
    assert!(!row.unassigned.contains(&key("n_box")));
}
