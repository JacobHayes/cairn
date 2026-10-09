//! D8, display state: the one state every surface shows for a node. One table of rows of the
//! precedence table over a hand-built journey, including the pending rule on a decision gated
//! on another decision (Gating, Kleene chains); the pending rule through containers and
//! settled reasons; the counts and the level beside `by_state`, which stays what it was; and
//! the narrowed `unassigned` (E1). Derived at 2026-10-06 unless a row says otherwise.
#![cfg(test)]

use crate::engine::support;

use std::collections::{BTreeMap, BTreeSet};

use DisplayState::{
    Active, Blocked, Conditional, Done, NotRelevant, Ready, Scheduled, Skipped, Snoozed,
};
use cairn_engine::{Derived, DerivedJourney, Graph, Records};
use cairn_schema::{
    Deployment, DisplayState, LevelQuery, NextQuery, NodeKind, Relevance, SnapshotScope, State,
    StatusSummary,
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
    steps.iter().fold(records.clone(), |records, step| {
        support::accepted(&records, step)
    })
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
const LATE: &str = "{key: n_late, id: late, kind: action, title: Late, relevant_when: {equals: {decision: n_ask, value: true}}}";
const GATED: &str = "{key: n_gated, id: gated, kind: milestone, title: Gated, auto_reach: true, requires: [n_first]}";
const THIRD: &str = "{key: n_third, id: third, kind: decision, title: Third, prompt: Third?, answer_type: boolean, relevant_when: {not: {equals: {decision: n_branch, value: true}}}}";
const READER: &str = "{key: n_reader, id: reader, kind: action, title: Reader, relevant_when: {equals: {decision: n_third, value: false}}}";

fn board() -> Records {
    support::journey(&add(&[
        ASK, BRANCH, INNER, PLAIN, FIRST, SECOND, GATE, STAGE, STEP_A, STEP_B, WALLED, WALL_KID,
        BOX, BOX_KID, NAP, DROP, FINISH, BEGUN, LATE, GATED, THIRD, READER,
    ]))
}

/// A step written in words: `done n_x`, `start n_x`, `reach n_x`, `reopen n_x`, `skip n_x`,
/// `yes n_decision`, `no n_decision`, `pin n_x 2026-10-05`, `snooze n_x 2026-10-09`.
fn step(text: &str) -> String {
    match text.split(' ').collect::<Vec<_>>().as_slice() {
        ["done", node] => transition(node, "complete"),
        [verb @ ("start" | "reach" | "reopen"), node] => transition(node, verb),
        ["skip", node] => skip(node),
        ["yes", decision] => answer(decision, "{boolean: true}"),
        ["no", decision] => answer(decision, "{boolean: false}"),
        ["pin", node, on] => pin(node, on),
        ["snooze", node, until] => snooze(node, &format!("{{date: \"{until}\"}}")),
        other => panic!("no step reads {other:?}"),
    }
}

/// D8, one row per fact of the precedence table: after the steps, at the day, the node shows
/// the state. Ready (a container waiting only on its unstarted children), active (a container
/// with work beneath it started), blocked (own gates only), done, skipped (through a skipped
/// container too), snoozed (beats started work beneath), scheduled, conditional (done once
/// finished), not relevant (beats done and skipped), and the pending rule: a node behind an
/// undecided decision is conditional, not ruled out, until the answers settle it.
#[test]
fn precedence_rows() {
    const DAY: &str = "2026-10-06";
    #[rustfmt::skip]
    let rows: [(&[&str], &str, &str, DisplayState); 36] = [
        (&[], DAY, "n_plain", Ready),
        (&[], DAY, "n_stage", Ready),
        (&[], DAY, "n_box", Ready),
        (&["done n_step_a", "done n_step_b"], DAY, "n_stage", Ready),
        (&["start n_begun"], DAY, "n_begun", Active),
        (&["start n_step_a"], DAY, "n_stage", Active),
        (&["start n_step_a", "done n_step_a"], DAY, "n_stage", Active),
        (&["start n_box_kid"], DAY, "n_box", Active),
        (&[], DAY, "n_second", Blocked),
        (&[], DAY, "n_wall_kid", Blocked),
        (&["done n_first"], DAY, "n_walled", Ready),
        (&["start n_wall_kid"], DAY, "n_walled", Blocked),
        (&["done n_finish"], DAY, "n_finish", Done),
        (&["yes n_ask"], DAY, "n_ask", Done),
        (&["done n_box_kid"], DAY, "n_box", Done),
        (&["reach n_gate"], DAY, "n_gate", Done),
        (&["pin n_gate 2026-10-05"], DAY, "n_gate", Done),
        (&["skip n_drop"], DAY, "n_drop", Skipped),
        (&["skip n_stage"], DAY, "n_step_a", Skipped),
        (&["snooze n_nap 2026-10-09"], DAY, "n_nap", Snoozed),
        (&["snooze n_nap 2026-10-09"], "2026-10-09", "n_nap", Ready),
        (&["start n_step_a", "snooze n_stage 2026-10-09"], DAY, "n_step_a", Snoozed),
        (&["pin n_gate 2026-10-20"], DAY, "n_gate", Scheduled),
        (&["pin n_gate 2026-10-20"], "2026-10-20", "n_gate", Done),
        (&["pin n_gated 2026-10-20"], DAY, "n_gated", Blocked),
        (&[], DAY, "n_branch", Conditional),
        (&["done n_late"], DAY, "n_late", Done),
        (&["no n_ask"], DAY, "n_branch", NotRelevant),
        (&["no n_ask"], DAY, "n_inner", NotRelevant),
        (&["done n_late", "no n_ask"], DAY, "n_late", NotRelevant),
        (&["skip n_late", "no n_ask"], DAY, "n_late", NotRelevant),
        (&[], DAY, "n_inner", Conditional),
        (&["yes n_ask"], DAY, "n_inner", Conditional),
        (&["yes n_ask", "yes n_branch"], DAY, "n_inner", Ready),
        (&["yes n_ask", "no n_branch"], DAY, "n_inner", NotRelevant),
        // A pending node can wait on a decision that is relevant now: reopening the first
        // decision leaves the third (relevant, answered) reading an unknown, so what reads
        // the third's answer may yet change.
        (&["no n_ask", "yes n_third", "reopen n_ask"], DAY, "n_reader", Conditional),
    ];
    for (words, today, node, expected) in rows {
        let steps: Vec<String> = words.iter().map(|text| step(text)).collect();
        let seen = Seen::of(&then(&board(), &steps), support::JOURNEY, today);
        assert_eq!(
            seen.shown(node),
            expected,
            "{node} after {words:?} on {today}"
        );
    }
    let ruled_out = Seen::test(&then(&board(), &[step("no n_ask")]));
    assert_eq!(
        ruled_out.stored("n_branch"),
        State::Open,
        "a not relevant decision keeps its recorded state"
    );
}

/// D8: pending carries through a chain and through a container, a
/// settled reason beside it keeps the node not relevant, and a skip above an undecided
/// decision that was already decided does not settle what reads it (D1a reaches only what is
/// still open).
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

/// D8: the count by display state agrees with the status summary, the snapshot, the next list
/// and the level on every fixture once its scenario has run; `by_state` is what it was: the
/// in-scope nodes by stored state.
#[test]
fn counts_agree_everywhere_and_by_state_is_unchanged() {
    for name in support::fixture_names() {
        let journey = support::scenario(&name).journey.to_string();
        let seen = Seen::of(&support::finished(&name), &journey, "2026-12-01");
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
        assert_eq!(summary.by_state, stored, "{name}");
        assert_eq!(snapshot.by_state, stored, "{name}");
        assert_eq!(summary.by_display_state, shown, "{name}");
        assert_eq!(snapshot.by_display_state, shown, "{name}");
        assert!(!shown.contains_key(&NotRelevant), "{name}");
        let next = seen
            .journey()
            .next(&NextQuery::default(), &BTreeSet::default())
            .unwrap();
        for row in &next.items {
            assert!(
                matches!(row.display_state, Ready | Active),
                "{name}: {} is {:?}",
                row.key,
                row.display_state
            );
            assert_eq!(
                row.display_state,
                seen.derived.display_state(&seen.graph, &row.key)
            );
        }
        let all = [
            NodeKind::Group,
            NodeKind::Action,
            NodeKind::Deliverable,
            NodeKind::Decision,
            NodeKind::Milestone,
        ];
        let level = seen
            .journey()
            .level(
                &LevelQuery::of_kinds(all.into_iter().collect(), None),
                &Deployment::default(),
            )
            .unwrap();
        for node in &level.nodes {
            assert_eq!(
                node.display_state,
                seen.derived.display_state(&seen.graph, &node.key),
                "{name}: {}",
                node.key
            );
        }
    }
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
    let row = seen.journey().snapshot(&SnapshotScope::default()).unwrap();
    assert!(row.unassigned.contains(&key("n_plain")));
    assert!(!row.unassigned.contains(&key("n_box")));
}
