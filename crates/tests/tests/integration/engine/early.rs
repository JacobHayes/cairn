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
    ConstraintSource, Date, DependencyVia, DoneEarly, NodeDates, Rejection, ViolationCode,
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

/// F6: starting is never guarded, so starting early is not a shortfall: not the work's, not
/// its prerequisite's, and not the milestone work waiting for its own date; but finishing it
/// before that date is still refused.
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
            "{}{}{}",
            pin("n_k", "2026-10-10"),
            transition("n_x", "start"),
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
