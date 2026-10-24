//! Work done early on purpose (F6, D8; ARCHITECTURE, Date network > Execution layer): started
//! work is held at its finish, so an early start is never a shortfall; a stage opening that
//! does not gate holds only work not yet started; and finished work is not held by what its
//! guard bypass accepted, its own date when the bypass accepted that, or a milestone's own
//! date. It reads done early on today's dates; every other constraint still holds it, so a
//! dependent finished normally whose prerequisite is reopened and finished after it stays a
//! shortfall. Hand-built journeys are created on 2026-10-06 and derived at that day.
#![cfg(test)]

use crate::engine::consequences::caused;
use crate::engine::support;

use cairn_engine::Records;
use cairn_schema::{
    Bypass, ConstraintSource, Date, DependencyVia, DoneEarly, Guard, GuardFailure, NodeDates,
    Rejection, ViolationCode,
};
use support::{add_nodes as add, key};

fn date(text: &str) -> Date {
    text.parse().unwrap()
}

fn dates_of(records: &Records, node: &str) -> NodeDates {
    let derived = support::derived(records, support::JOURNEY);
    let graph = support::journey_graph(records, support::JOURNEY);
    derived.dates().node(&graph, &key(node))
}

fn short(records: &Records, node: &str) -> Option<u32> {
    support::derived(records, support::JOURNEY)
        .dates()
        .shortfall_days(&key(node))
}

fn early(records: &Records, node: &str) -> Option<DoneEarly> {
    dates_of(records, node).done_early
}

fn pin(node: &str, on: &str) -> String {
    format!("- op: set_pin\n  node: {node}\n  date: \"{on}\"\n")
}

fn transition(node: &str, transition: &str) -> String {
    format!("- op: transition\n  node: {node}\n  transition: {transition}\n")
}

fn recorded(node: &str, end: &str, on: &str) -> String {
    format!("- op: set_recorded_date\n  node: {node}\n  end: {end}\n  date: \"{on}\"\n")
}

fn bypass(node: &str, guard: &str) -> String {
    format!(
        "- op: apply_override\n  node: {node}\n  override: {{guard_bypass: {{guards: [{guard}], reason: Done ahead on purpose.}}}}\n"
    )
}

/// A bypass with no reason, as a person leaves it out.
fn bypass_without_reason(node: &str, guard: &str) -> String {
    format!(
        "- op: apply_override\n  node: {node}\n  override: {{guard_bypass: {{guards: [{guard}]}}}}\n"
    )
}

fn dependency(node: &str, requires: &str, via: DependencyVia) -> ConstraintSource {
    ConstraintSource::Dependency {
        node: key(node),
        requires: key(requires),
        via,
    }
}

/// F6: work completed ahead of a prerequisite with a recorded reason is done early, not
/// short, and holds its prerequisite's due where it was (on main the prerequisite's due fell
/// to the completion day and it read short). The exemption is narrow: the work is still short
/// of its own deadline.
#[test]
fn a_bypassed_early_completion_is_done_early_not_short() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_ma, id: ma, kind: milestone, title: Ma}",
            "{key: n_mb, id: mb, kind: milestone, title: Mb}",
            "{key: n_r, id: r, kind: action, title: R, estimate: 3, due_by: {before: n_ma}}",
            "{key: n_x, id: x, kind: action, title: X, requires: [n_r], due_by: {before: n_mb}}",
        ])),
        &format!("{}{}", pin("n_ma", "2026-11-20"), pin("n_mb", "2026-10-03")),
    );
    let completing = format!(
        "{}{}",
        bypass("n_x", "deps_done"),
        transition("n_x", "complete"),
    );
    let consequences = caused(&records, &completing, "2026-10-06");
    assert!(
        consequences
            .shortfalls
            .iter()
            .all(|found| found.node != key("n_r")),
        "completing ahead of it does not make the prerequisite short"
    );
    let done = support::accepted(&records, &completing);
    assert_eq!(short(&done, "n_r"), None);
    assert_eq!(
        dates_of(&done, "n_r").due.map(|due| due.date),
        Some(date("2026-11-20")),
        "its due stays where its own deadline puts it"
    );
    assert_eq!(
        early(&done, "n_x"),
        Some(DoneEarly {
            days: 3,
            date: date("2026-10-09"),
            source: dependency("n_x", "n_r", DependencyVia::Explicit),
        })
    );
    assert_eq!(
        short(&done, "n_x"),
        Some(3),
        "finished three days past its own deadline"
    );
}

/// F6: a bypass for one dependency leaves the others holding the work. Work under a group
/// that requires two things, finished with a bypass for one: when the other is reopened and
/// finished after it, that one is short (shown as inherited from the group), the bypassed one
/// is not, and the work is done early ahead of the bypassed one.
#[test]
fn a_bypass_for_one_dependency_keeps_the_others() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_r, id: r, kind: action, title: R, estimate: 3}",
            "{key: n_e, id: e, kind: action, title: E, estimate: 2}",
            "{key: n_p, id: p, kind: group, title: P, requires: [n_r, n_e]}",
            "{key: n_x, id: x, parent: n_p, kind: action, title: X}",
        ])),
        &format!(
            "{}{}{}{}{}",
            transition("n_e", "complete"),
            recorded("n_e", "finish", "2026-10-03"),
            bypass("n_x", "deps_done"),
            transition("n_x", "complete"),
            recorded("n_x", "finish", "2026-10-04"),
        ),
    );
    assert_eq!(
        short(&records, "n_e"),
        None,
        "finished before it was needed"
    );
    let reopened = support::accepted(
        &records,
        &format!(
            "{}{}",
            transition("n_e", "reopen"),
            transition("n_e", "complete")
        ),
    );
    let shortfall = dates_of(&reopened, "n_e").shortfall.unwrap();
    assert_eq!(
        shortfall.shortfall_days, 2,
        "finished the 6th, needed by the 4th"
    );
    assert_eq!(
        shortfall.chain.constraints[0].source,
        dependency(
            "n_x",
            "n_e",
            DependencyVia::Inherited {
                ancestor: key("n_p")
            }
        ),
        "the expanded constraint still says where the requirement came from"
    );
    assert_eq!(short(&reopened, "n_r"), None);
    let found = early(&reopened, "n_x").unwrap();
    assert_eq!(
        found.days, 5,
        "finished the 4th, ahead of R's earliest finish, the 9th"
    );
    assert_eq!(
        found.source,
        dependency(
            "n_x",
            "n_r",
            DependencyVia::Inherited {
                ancestor: key("n_p")
            }
        )
    );
}

/// F6: a prerequisite reopened and finished after its dependent was finished normally is a
/// shortfall on the prerequisite, and the dependent is not done early.
#[test]
fn a_reopened_prerequisite_finished_after_its_dependent_stays_short() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_r, id: r, kind: action, title: R, estimate: 2}",
            "{key: n_x, id: x, kind: action, title: X, requires: [n_r]}",
        ])),
        &format!(
            "{}{}{}{}",
            transition("n_r", "complete"),
            recorded("n_r", "finish", "2026-10-03"),
            transition("n_x", "complete"),
            recorded("n_x", "finish", "2026-10-04"),
        ),
    );
    assert_eq!(short(&records, "n_r"), None);
    let reopened = support::accepted(
        &records,
        &format!(
            "{}{}",
            transition("n_r", "reopen"),
            transition("n_r", "complete")
        ),
    );
    assert_eq!(short(&reopened, "n_r"), Some(2));
    assert_eq!(early(&reopened, "n_x"), None);
}

/// F4, F6: work done inside a stage that does not wait for its opening is done early, with
/// and without a recorded start, and neither it nor the opening nor the stage is short; what
/// has not started still waits for the opening; and once everything inside has finished, the
/// stage no longer carries the opening on to what depends on it.
#[test]
fn early_work_in_a_stage_that_does_not_wait_is_done_early() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_opens, id: opens, kind: milestone, title: Opens}",
            "{key: n_stage, id: stage, kind: group, title: Stage, opens_at: n_opens, gates: false}",
            "{key: n_prep, id: prep, parent: n_stage, kind: action, title: Prep}",
            "{key: n_pack, id: pack, parent: n_stage, kind: action, title: Pack}",
            "{key: n_later, id: later, parent: n_stage, kind: action, title: Later}",
            "{key: n_after, id: after, kind: action, title: After, requires: [n_stage]}",
        ])),
        &pin("n_opens", "2026-10-23"),
    );
    let early_work = support::accepted(
        &records,
        &format!(
            "{}{}{}",
            transition("n_prep", "complete"),
            transition("n_pack", "start"),
            transition("n_pack", "complete"),
        ),
    );
    for node in ["n_prep", "n_pack", "n_opens", "n_stage"] {
        assert_eq!(short(&early_work, node), None, "{node}");
    }
    for node in ["n_prep", "n_pack"] {
        let found = early(&early_work, node).unwrap();
        assert_eq!((found.days, found.date), (17, date("2026-10-23")), "{node}");
        assert_eq!(
            found.source,
            dependency(
                node,
                "n_opens",
                DependencyVia::StageOpening {
                    group: key("n_stage")
                }
            ),
            "{node}"
        );
    }
    assert_eq!(
        dates_of(&early_work, "n_later")
            .earliest_start
            .map(|bound| bound.date),
        Some(date("2026-10-23")),
        "the opening still dates work not yet started"
    );
    let all = support::accepted(
        &early_work,
        &format!(
            "{}{}",
            transition("n_later", "complete"),
            transition("n_after", "complete")
        ),
    );
    for node in ["n_after", "n_stage", "n_opens", "n_later"] {
        assert_eq!(short(&all, node), None, "{node}");
    }
    let edited = support::accepted(&all, &recorded("n_pack", "finish", "2026-10-25"));
    assert_eq!(
        early(&edited, "n_pack"),
        None,
        "finished after the opening: nothing it was finished ahead of is violated"
    );
}

/// F6: a start frees nothing, so starting early (with a bypass) is not a shortfall: not the
/// work's, not its prerequisite's, and not the milestone work waiting for its own date; but
/// finishing it before that date is still refused.
#[test]
fn starting_early_is_not_a_shortfall() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_r, id: r, kind: action, title: R, estimate: 3}",
            "{key: n_x, id: x, kind: action, title: X, requires: [n_r]}",
            "{key: n_k, id: k, kind: milestone, title: K}",
            "{key: n_z, id: z, kind: action, title: Z, not_before: {after: n_k}}",
        ])),
        &format!(
            "{}{}{}{}{}",
            pin("n_k", "2026-10-10"),
            bypass_without_reason("n_x", "deps_done"),
            transition("n_x", "start"),
            bypass_without_reason("n_z", "date_arrived"),
            transition("n_z", "start"),
        ),
    );
    for node in ["n_x", "n_r", "n_z", "n_k"] {
        assert_eq!(short(&records, node), None, "{node}");
    }
    let derived = support::derived(&records, support::JOURNEY);
    assert_eq!(
        derived.dates().earliest_finish(&key("n_x")),
        derived.dates().earliest_finish(&key("n_r")),
        "started work still finishes after what it waits for"
    );
    let Err(Rejection::Invalid { violations }) =
        support::journey_patch(&records, &transition("n_z", "complete"))
    else {
        panic!("completing before its own date is refused");
    };
    assert_eq!(violations.as_slice()[0].code, ViolationCode::GuardFailed);
}

/// F6: a milestone marked done ahead of what it waits for, a decision answered with a
/// reason ahead of its prerequisite, and a milestone done before its own not-before date are
/// done early, and none holds anything it waited for short.
#[test]
fn an_early_milestone_and_decision() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_r, id: r, kind: action, title: R, estimate: 3}",
            "{key: n_m, id: m, kind: milestone, title: M, requires: [n_r]}",
            "{key: n_d, id: d, kind: decision, title: D, prompt: Which?, answer_type: text, requires: [n_r]}",
            "{key: n_k, id: k, kind: milestone, title: K}",
            "{key: n_n, id: n, kind: milestone, title: N, not_before: {after: n_k, offset: 5}}",
        ])),
        &pin("n_k", "2026-10-08"),
    );
    let done = support::accepted(
        &records,
        &format!(
            "{}{}{}{}{}",
            bypass("n_m", "deps_done"),
            transition("n_m", "complete"),
            bypass("n_d", "deps_done"),
            "- op: answer\n  decision: n_d\n  value: {text: Soon.}\n",
            transition("n_n", "complete"),
        ),
    );
    for node in ["n_r", "n_m", "n_d", "n_k", "n_n"] {
        assert_eq!(short(&done, node), None, "{node}");
    }
    for node in ["n_m", "n_d"] {
        let found = early(&done, node).unwrap();
        assert_eq!((found.days, found.date), (3, date("2026-10-09")), "{node}");
        assert_eq!(
            found.source,
            dependency(node, "n_r", DependencyVia::Explicit)
        );
    }
    let found = early(&done, "n_n").unwrap();
    assert_eq!((found.days, found.date), (7, date("2026-10-13")));
    assert_eq!(
        found.source,
        ConstraintSource::NotBefore { node: key("n_n") }
    );
}

/// F6, A8: work finished before its own not-before date with a recorded reason is done early
/// ahead of that date, and the milestone it waits for is not held short by it.
#[test]
fn finishing_before_the_own_date_with_a_reason_is_done_early() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_k, id: k, kind: milestone, title: K}",
            "{key: n_setup, id: setup, kind: action, title: Setup, not_before: {after: n_k}}",
        ])),
        &pin("n_k", "2026-10-10"),
    );
    let done = support::accepted(
        &records,
        &format!(
            "{}{}",
            bypass("n_setup", "date_arrived"),
            transition("n_setup", "complete")
        ),
    );
    assert_eq!(short(&done, "n_k"), None);
    assert_eq!(short(&done, "n_setup"), None);
    let found = early(&done, "n_setup").unwrap();
    assert_eq!((found.days, found.date), (4, date("2026-10-10")));
    assert_eq!(
        found.source,
        ConstraintSource::NotBefore {
            node: key("n_setup")
        }
    );
}

/// F6: the days are counted to a milestone's date as the person sees it (its due, when
/// nothing pins it), not to the earliest it could happen: work finished before an opening
/// whose earliest is two days out and whose date is two weeks out is 14 days early.
#[test]
fn the_days_are_counted_to_the_milestones_date_as_shown() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_launch, id: launch, kind: milestone, title: Launch}",
            "{key: n_opens, id: opens, kind: milestone, title: Opens, not_before: {after: journey.created_at, offset: 2}, due_by: {before: n_launch}}",
            "{key: n_stage, id: stage, kind: group, title: Stage, opens_at: n_opens, gates: false}",
            "{key: n_prep, id: prep, parent: n_stage, kind: action, title: Prep}",
        ])),
        &format!(
            "{}{}",
            pin("n_launch", "2026-10-20"),
            transition("n_prep", "complete")
        ),
    );
    let found = early(&records, "n_prep").unwrap();
    assert_eq!((found.days, found.date), (14, date("2026-10-20")));
}

/// F6: work finished ahead of a milestone that completes by its date is early, not the
/// milestone short. Left held, finishing the work would cap the milestone's latest date at the
/// finish, so it would read reached, the guard would see nothing open to record, and the
/// milestone would read short; the work is done early instead, by the days to the milestone's
/// date as shown.
#[test]
fn work_finished_ahead_of_an_auto_done_milestone_is_done_early() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_s, id: s, kind: milestone, title: S}",
            "{key: n_m, id: m, kind: milestone, title: M, auto_done: true, not_before: {after: n_s, offset: 2}}",
            "{key: n_end, id: end, kind: milestone, title: End}",
            "{key: n_g, id: g, kind: group, title: G, opens_at: n_m, closes_at: n_end}",
            "{key: n_x, id: x, parent: n_g, kind: action, title: X, estimate: 1}",
        ])),
        &format!(
            "{}{}{}{}",
            transition("n_s", "complete"),
            pin("n_end", "2026-10-20"),
            bypass("n_x", "deps_done"),
            transition("n_x", "complete"),
        ),
    );
    for node in ["n_x", "n_m", "n_s"] {
        assert_eq!(short(&records, node), None, "{node}");
    }
    let found = early(&records, "n_x").expect("done early");
    assert_eq!(
        found.days, 14,
        "from the completion on Oct 6 to M as shown, Oct 20"
    );
    assert_eq!(found.date, date("2026-10-20"));
    assert_eq!(
        found.source,
        dependency(
            "n_x",
            "n_m",
            DependencyVia::StageOpening { group: key("n_g") }
        )
    );
}

/// F6: an `auto_done` milestone that already reads reached (its only date is the due its
/// dependent gives it, passed) stays reached when the dependent is completed: finishing the
/// work is not early of it, and completing needs no bypass.
#[test]
fn work_after_a_reached_auto_done_milestone_is_completed_without_a_bypass() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_m, id: m, kind: milestone, title: M, auto_done: true}",
            "{key: n_x, id: x, kind: action, title: X, requires: [n_m]}",
        ])),
        &format!(
            "{}{}",
            pin("n_x", "2026-10-05"),
            transition("n_x", "complete")
        ),
    );
    for node in ["n_x", "n_m"] {
        assert_eq!(short(&records, node), None, "{node}");
    }
    assert_eq!(early(&records, "n_x"), None);
}

// Early work is one act: a start or a finish ahead of what the node waits on takes a guard
// bypass, whose reason is optional; the engine records what it went ahead of.

fn stored_bypass(records: &Records, node: &str) -> Option<Bypass> {
    support::journey_graph(records, support::JOURNEY)
        .document()
        .state
        .overrides
        .get(&key(node))
        .and_then(|overrides| overrides.bypass.clone())
}

fn derived_of(records: &Records, node: &str) -> cairn_schema::NodeDerived {
    let graph = support::journey_graph(records, support::JOURNEY);
    support::derived(records, support::JOURNEY).node_derived(&graph, &key(node))
}

fn bypass_with_reason(node: &str, guard: &str, reason: Option<&str>) -> String {
    reason.map_or_else(
        || bypass_without_reason(node, guard),
        |reason| {
            format!(
                "- op: apply_override\n  node: {node}\n  override: {{guard_bypass: {{guards: [{guard}], reason: {reason}}}}}\n"
            )
        },
    )
}

/// A prerequisite and the work that waits on it.
fn chain() -> Records {
    support::journey(&add(&[
        "{key: n_a, id: a, kind: action, title: A, estimate: 3}",
        "{key: n_b, id: b, kind: action, title: B, requires: [n_a]}",
    ]))
}

/// Each early move's graph: its name, nodes, the node that goes early, the guard its bypass
/// names, and the setup written first.
fn scenarios() -> Vec<Scenario> {
    [dependency_scenarios(), other_scenarios()].concat()
}

/// A name, nodes, the node that goes early, the guard its bypass names, and the setup.
type Scenario = (
    &'static str,
    Vec<&'static str>,
    &'static str,
    &'static str,
    String,
);

/// The scenarios of moves ahead of a dependency, inherited requirement or gating opening.
fn dependency_scenarios() -> Vec<Scenario> {
    vec![
        (
            "explicit dependency",
            vec![
                "{key: n_r, id: r, kind: action, title: R, estimate: 3}",
                "{key: n_x, id: x, kind: action, title: X, requires: [n_r]}",
            ],
            "n_x",
            "deps_done",
            String::new(),
        ),
        (
            "inherited requirement",
            vec![
                "{key: n_r, id: r, kind: action, title: R, estimate: 3}",
                "{key: n_p, id: p, kind: group, title: P, requires: [n_r]}",
                "{key: n_x, id: x, parent: n_p, kind: action, title: X}",
            ],
            "n_x",
            "deps_done",
            String::new(),
        ),
        (
            "gating stage opening",
            vec![
                "{key: n_o, id: o, kind: milestone, title: O}",
                "{key: n_s, id: s, kind: group, title: S, opens_at: n_o}",
                "{key: n_x, id: x, parent: n_s, kind: action, title: X}",
            ],
            "n_x",
            "deps_done",
            String::new(),
        ),
    ]
}

/// The scenarios of a container, a decision, a milestone and an own date.
fn other_scenarios() -> Vec<Scenario> {
    vec![
        (
            "container ahead of its children",
            vec![
                "{key: n_c, id: c, kind: deliverable, title: C}",
                "{key: n_x, id: x, parent: n_c, kind: action, title: X}",
            ],
            "n_c",
            "deps_done",
            String::new(),
        ),
        (
            "decision answered ahead",
            vec![
                "{key: n_r, id: r, kind: action, title: R, estimate: 3}",
                "{key: n_d, id: d, kind: decision, title: D, prompt: Which?, answer_type: text, requires: [n_r]}",
            ],
            "n_d",
            "deps_done",
            String::new(),
        ),
        (
            "milestone done ahead",
            vec![
                "{key: n_r, id: r, kind: action, title: R, estimate: 3}",
                "{key: n_m, id: m, kind: milestone, title: M, requires: [n_r]}",
            ],
            "n_m",
            "deps_done",
            String::new(),
        ),
        (
            "before its own date",
            vec![
                "{key: n_k, id: k, kind: milestone, title: K}",
                "{key: n_x, id: x, kind: action, title: X, not_before: {after: n_k}}",
            ],
            "n_x",
            "date_arrived",
            pin("n_k", "2026-10-10"),
        ),
    ]
}

/// The reason changes nothing derived: the same early moves with and without one give equal
/// dates, shortfalls and done-early readings, and each records the same failures.
#[test]
fn a_reason_changes_nothing_derived() {
    for (name, nodes, node, guard, setup) in scenarios() {
        let records = {
            let journey = support::journey(&add(&nodes));
            if setup.is_empty() {
                journey
            } else {
                support::accepted(&journey, &setup)
            }
        };
        let finish = if node == "n_d" {
            "- op: answer\n  decision: n_d\n  value: {text: Ahead.}\n".to_owned()
        } else {
            transition(node, "complete")
        };
        let mut seen = Vec::new();
        for reason in [None, Some("Done ahead on purpose.")] {
            let done = support::accepted(
                &records,
                &format!("{}{finish}", bypass_with_reason(node, guard, reason)),
            );
            let stored = stored_bypass(&done, node).unwrap_or_else(|| panic!("{name}: recorded"));
            assert_eq!(stored.reason.is_some(), reason.is_some(), "{name}");
            assert!(!stored.failures.is_empty(), "{name}: what it went ahead of");
            let nodes: Vec<String> = support::graph(&done)
                .nodes
                .as_map()
                .keys()
                .map(ToString::to_string)
                .collect();
            let readings: Vec<_> = nodes
                .iter()
                .map(|each| (dates_of(&done, each).done_early, short(&done, each)))
                .collect();
            seen.push((readings, stored.failures));
        }
        assert_eq!(seen[0], seen[1], "{name}: the reason changes nothing");
    }
}

/// Starting work that waits is refused with a bypassable guard for each thing it waits on;
/// a bypass without a reason starts it, records both, and reads started early. A container
/// does not wait on its own children.
#[test]
fn starting_ahead_is_guarded_and_recorded() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_a, id: a, kind: action, title: A, estimate: 3}",
            "{key: n_k, id: k, kind: milestone, title: K}",
            "{key: n_b, id: b, kind: action, title: B, requires: [n_a], not_before: {after: n_k}}",
            "{key: n_c, id: c, kind: deliverable, title: C}",
            "{key: n_c1, id: c1, parent: n_c, kind: action, title: C1}",
        ])),
        &pin("n_k", "2026-10-10"),
    );
    assert_eq!(
        derived_of(&records, "n_b").start_guards,
        [Guard::DepsDone, Guard::DateArrived].into_iter().collect(),
        "the engine says what a start would go ahead of"
    );
    assert!(
        derived_of(&records, "n_c").start_guards.is_empty(),
        "a container's children are not what its start waits on"
    );
    let Err(Rejection::Invalid { violations }) =
        support::journey_patch(&records, &transition("n_b", "start"))
    else {
        panic!("a plain start of waiting work is refused");
    };
    let guards: std::collections::BTreeSet<_> = violations
        .as_slice()
        .iter()
        .map(|found| {
            assert_eq!(found.code, ViolationCode::GuardFailed);
            found.bypassable.unwrap()
        })
        .collect();
    assert_eq!(
        guards,
        [Guard::DepsDone, Guard::DateArrived].into_iter().collect()
    );
    let started = support::accepted(
        &records,
        &format!(
            "{}{}{}",
            bypass_without_reason("n_b", "deps_done")
                .replace("[deps_done]", "[deps_done, date_arrived]"),
            transition("n_b", "start"),
            transition("n_c", "start"),
        ),
    );
    let stored = stored_bypass(&started, "n_b").unwrap();
    assert!(stored.reason.is_none());
    assert_eq!(
        stored.failures,
        [
            GuardFailure::OpenDependency(key("n_a")),
            GuardFailure::TooEarly(date("2026-10-10")),
        ]
        .into_iter()
        .collect()
    );
    assert!(derived_of(&started, "n_b").started_early);
    assert!(
        !derived_of(&started, "n_c").started_early,
        "a container started with its children open went ahead of nothing"
    );
    assert!(stored_bypass(&started, "n_c").is_none());
    assert_eq!(short(&started, "n_b"), None);
}

/// Started normally, then its prerequisite is reopened: the work did not go ahead of anything,
/// so it does not read started early.
#[test]
fn a_start_before_a_reopening_is_not_early() {
    let records = support::accepted(&chain(), &transition("n_a", "complete"));
    let started = support::accepted(&records, &transition("n_b", "start"));
    let reopened = support::accepted(&started, &transition("n_a", "reopen"));
    assert_eq!(
        derived_of(&reopened, "n_b")
            .waits_on
            .entries
            .as_slice()
            .len(),
        1
    );
    assert!(!derived_of(&reopened, "n_b").started_early);
}

/// Started ahead, then finished normally once the prerequisite is done: no record, nothing
/// short, not done early. Reopen the prerequisite: the work is stale while it is open, and
/// when it finishes after the work it is short, and the work is no longer stale.
#[test]
fn started_ahead_then_finished_normally_relies_on_its_prerequisite() {
    let started = support::accepted(
        &chain(),
        &format!(
            "{}{}{}",
            bypass_without_reason("n_b", "deps_done"),
            transition("n_b", "start"),
            recorded("n_b", "start", "2026-10-02"),
        ),
    );
    assert!(derived_of(&started, "n_b").started_early);
    let both = support::accepted(
        &started,
        &format!(
            "{}{}{}{}",
            transition("n_a", "complete"),
            recorded("n_a", "finish", "2026-10-03"),
            transition("n_b", "complete"),
            recorded("n_b", "finish", "2026-10-04"),
        ),
    );
    assert!(
        stored_bypass(&both, "n_b").is_none(),
        "the finish went ahead of nothing"
    );
    assert_eq!((short(&both, "n_a"), short(&both, "n_b")), (None, None));
    assert_eq!(early(&both, "n_b"), None);
    let reopened = support::accepted(&both, &transition("n_a", "reopen"));
    assert!(
        !derived_of(&reopened, "n_b").stale.is_empty(),
        "stale while A is open"
    );
    let finished = support::accepted(&reopened, &transition("n_a", "complete"));
    assert!(
        short(&finished, "n_a").is_some(),
        "A finished after B relied on it"
    );
    assert!(derived_of(&finished, "n_b").stale.is_empty());
}

/// Stopping clears the record of the start, so starting again once the prerequisite is done
/// is not early.
#[test]
fn stopping_clears_the_record_of_a_start() {
    let started = support::accepted(
        &chain(),
        &format!(
            "{}{}",
            bypass_without_reason("n_b", "deps_done"),
            transition("n_b", "start")
        ),
    );
    let stopped = support::accepted(&started, &transition("n_b", "stop"));
    assert!(stored_bypass(&stopped, "n_b").is_none());
    let again = support::accepted(
        &stopped,
        &format!(
            "{}{}",
            transition("n_a", "complete"),
            transition("n_b", "start")
        ),
    );
    assert!(stored_bypass(&again, "n_b").is_none());
    assert!(!derived_of(&again, "n_b").started_early);
}

/// Finished ahead of the prerequisite with no reason: the prerequisite later finishes,
/// reopens and finishes after the work, and neither is short or stale.
#[test]
fn an_accepted_dependency_stays_accepted() {
    let done = support::accepted(
        &chain(),
        &format!(
            "{}{}{}",
            bypass_without_reason("n_b", "deps_done"),
            transition("n_b", "complete"),
            recorded("n_b", "finish", "2026-10-04"),
        ),
    );
    let again = support::accepted(
        &done,
        &format!(
            "{}{}{}{}",
            transition("n_a", "complete"),
            recorded("n_a", "finish", "2026-10-05"),
            transition("n_a", "reopen"),
            transition("n_a", "complete"),
        ),
    );
    assert_eq!(short(&again, "n_a"), None);
    assert!(derived_of(&again, "n_b").stale.is_empty());
}

/// Finished with only its date excused, the dependency it waited for still counts: when it
/// is reopened and finished after the work, it is short.
#[test]
fn excusing_only_the_date_leaves_the_dependency_holding() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_k, id: k, kind: milestone, title: K}",
            "{key: n_a, id: a, kind: action, title: A, estimate: 2}",
            "{key: n_b, id: b, kind: action, title: B, requires: [n_a], not_before: {after: n_k}}",
        ])),
        &format!(
            "{}{}{}",
            pin("n_k", "2026-10-20"),
            transition("n_a", "complete"),
            recorded("n_a", "finish", "2026-10-03"),
        ),
    );
    let done = support::accepted(
        &records,
        &format!(
            "{}{}{}",
            bypass_without_reason("n_b", "date_arrived"),
            transition("n_b", "complete"),
            recorded("n_b", "finish", "2026-10-04"),
        ),
    );
    let reopened = support::accepted(
        &done,
        &format!(
            "{}{}",
            transition("n_a", "reopen"),
            transition("n_a", "complete")
        ),
    );
    assert!(short(&reopened, "n_a").is_some());
}

/// A completion bypassing a missing artifact, a note, an unticked item or a missing
/// breakdown needs no reason, and records the failure.
#[test]
fn evidence_bypasses_need_no_reason() {
    let records = support::journey(&add(&[
        "{key: n_art, id: art, kind: deliverable, title: Art, requires_artifact: true}",
        "{key: n_note, id: note, kind: action, title: Note, requires_note: true}",
        "{key: n_list, id: list, kind: action, title: List, checklist: [{id: one, title: One}], requires_checklist: true}",
        "{key: n_hold, id: hold, kind: action, title: Hold, placeholder: true}",
    ]));
    for (node, guard, failure) in [
        ("n_art", "has_artifact", GuardFailure::MissingArtifact),
        ("n_note", "has_note", GuardFailure::MissingNote),
        (
            "n_list",
            "all_ticked",
            GuardFailure::Unticked("one".parse().unwrap()),
        ),
        ("n_hold", "broken_down", GuardFailure::NotBrokenDown),
    ] {
        let done = support::accepted(
            &records,
            &format!(
                "{}{}",
                bypass_without_reason(node, guard),
                transition(node, "complete")
            ),
        );
        let stored = stored_bypass(&done, node).unwrap_or_else(|| panic!("{node}"));
        assert!(stored.reason.is_none(), "{node}");
        assert!(stored.failures.contains(&failure), "{node}");
    }
}
