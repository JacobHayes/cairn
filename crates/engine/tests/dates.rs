//! The execution layer (A8, F1 to F7; ARCHITECTURE, Date network) at the fixed clock (today
//! is 2026-10-06; the test journey was created that day): bounds both ways from pins,
//! actuals, and today, each with its chain; effective dates; slack; `overdue`; and
//! `shortfall` with its chain, never a rejection.
#![cfg(test)]

mod support;

use cairn_engine::Records;
use cairn_schema::{ConstraintSource, Date, DateOrigin, FixedBy, Instant, InstantPoint, NodeDates};
use support::key;

/// `add_node` mutations, one per node written as a YAML flow mapping.
fn add(nodes: &[&str]) -> String {
    let mut mutations = String::new();
    for node in nodes {
        mutations.push_str("- op: add_node\n  node: ");
        mutations.push_str(node);
        mutations.push('\n');
    }
    mutations
}

fn date(text: &str) -> Date {
    text.parse().unwrap()
}

/// The test journey's dates for a node, with chains.
fn dates_of(records: &Records, node: &str) -> NodeDates {
    let derived = support::derived(records, support::JOURNEY);
    let graph = support::journey_graph(records, support::JOURNEY);
    derived.dates().node(&graph, &key(node))
}

fn bound(found: Option<&cairn_schema::Bound>) -> Option<Date> {
    found.map(|bound| bound.date)
}

/// What fixes a chain's ends.
fn fixers(chain: &cairn_schema::Chain) -> Vec<FixedBy> {
    chain.fixed.iter().map(|fixed| fixed.fixed_by).collect()
}

const MEETING: &str = "{key: n_meeting, id: meeting, kind: milestone, title: Meeting}";

/// A8, F3: "the report is due 14 days before the meeting" caps the report from a pinned
/// meeting, and holds an unpinned meeting back from the report's earliest finish.
#[test]
fn a_rule_bounds_both_of_its_ends() {
    let report = "{key: n_report, id: report, kind: deliverable, title: Report, estimate: 3, due_by: {before: n_meeting, offset: 14}}";
    let records = support::journey(&add(&[MEETING, report]));
    let meeting = dates_of(&records, "n_meeting");
    assert_eq!(
        bound(meeting.earliest_start.as_ref()),
        Some(date("2026-10-23"))
    );
    assert_eq!(meeting.due, None, "nothing caps an unpinned meeting");
    let pinned = support::accepted(
        &records,
        "- op: set_pin\n  node: n_meeting\n  date: \"2026-11-20\"\n",
    );
    let report = dates_of(&pinned, "n_report");
    assert_eq!(bound(report.due.as_ref()), Some(date("2026-11-06")));
    assert_eq!(
        bound(report.latest_start.as_ref()),
        Some(date("2026-11-03"))
    );
    assert_eq!(report.slack_days, Some(28));
    let chain = &report.latest_start.unwrap().chain;
    assert_eq!(fixers(chain), [FixedBy::Pin]);
    let sources: Vec<_> = chain.constraints.iter().map(|c| c.source.clone()).collect();
    assert_eq!(
        sources,
        [
            ConstraintSource::Estimate {
                node: key("n_report")
            },
            ConstraintSource::DueBy {
                node: key("n_report")
            },
        ]
    );
}

/// A8: several sources are several constraints (the earliest cap wins), the offset defaults
/// to zero, `not_before` gives an earliest start, and `created_at` needs no milestone.
#[test]
fn rules_measure_from_milestones_and_created_at() {
    let records = support::journey(&add(&[
        MEETING,
        "{key: n_kickoff, id: kickoff, kind: milestone, title: Kickoff}",
        "{key: n_both, id: both, kind: action, title: Both, due_by: {before: [n_meeting, n_kickoff]}}",
        "{key: n_after, id: after, kind: action, title: After, not_before: {after: n_kickoff, offset: 3}}",
        "{key: n_soon, id: soon, kind: action, title: Soon, due_by: {after: journey.created_at, offset: 3}}",
    ]));
    let records = support::accepted(
        &records,
        "- op: set_pin\n  node: n_meeting\n  date: \"2026-11-20\"\n\
- op: set_pin\n  node: n_kickoff\n  date: \"2026-11-01\"\n",
    );
    assert_eq!(
        bound(dates_of(&records, "n_both").due.as_ref()),
        Some(date("2026-11-01"))
    );
    let after = dates_of(&records, "n_after");
    assert_eq!(
        bound(after.earliest_start.as_ref()),
        Some(date("2026-11-04"))
    );
    let soon = dates_of(&records, "n_soon");
    assert_eq!(bound(soon.due.as_ref()), Some(date("2026-10-09")));
    let fixed = &soon.due.unwrap().chain.fixed;
    assert_eq!(fixed[0].instant, Instant::CreatedAt);
    assert_eq!(fixed[0].fixed_by, FixedBy::Actual, "created_at is a fact");
}

/// F3: "why is this due Tuesday?" The example's final report is due when its review stage
/// closes, at the decision meeting its date decision pins, and must start three days before.
#[test]
fn the_examples_report_is_due_by_its_chain() {
    let records = support::vendor_after(2);
    let derived = support::derived(&records, "j_vendor_eval");
    let graph = support::journey_graph(&records, "j_vendor_eval");
    let report = derived.dates().node(&graph, &key("n_final_report"));
    assert_eq!(bound(report.due.as_ref()), Some(date("2026-11-20")));
    let latest = report.latest_start.unwrap();
    assert_eq!(latest.date, date("2026-11-17"));
    let sources: Vec<_> = latest
        .chain
        .constraints
        .iter()
        .map(|c| c.source.clone())
        .collect();
    assert_eq!(
        sources,
        [
            ConstraintSource::Estimate {
                node: key("n_final_report")
            },
            ConstraintSource::Containment {
                parent: key("n_final_review"),
                child: key("n_final_report")
            },
            ConstraintSource::Estimate {
                node: key("n_final_review")
            },
            ConstraintSource::StageClose {
                group: key("n_final_review")
            },
        ]
    );
    let end = latest.chain.fixed.last().unwrap();
    assert_eq!(end.date, date("2026-11-20"));
    assert_eq!(end.fixed_by, FixedBy::Pin);
    let meeting = derived
        .dates()
        .effective_date(&key("n_decision_meeting"))
        .unwrap();
    assert_eq!(
        (meeting.date, meeting.origin),
        (date("2026-11-20"), DateOrigin::Pin)
    );
}

/// Containment (planted bug): a child finishes before its parent's own work starts, so a
/// pinned parent with work of its own caps its child by that work.
#[test]
fn a_child_finishes_before_its_parents_own_work() {
    let records = support::journey(&add(&[
        "{key: n_parent, id: parent, kind: deliverable, title: Parent, estimate: 4}",
        "{key: n_child, id: child, parent: n_parent, kind: action, title: Child, estimate: 2}",
    ]));
    let records = support::accepted(
        &records,
        "- op: set_pin\n  node: n_parent\n  date: \"2026-11-10\"\n",
    );
    let child = dates_of(&records, "n_child");
    assert_eq!(bound(child.due.as_ref()), Some(date("2026-11-06")));
    assert_eq!(bound(child.latest_start.as_ref()), Some(date("2026-11-04")));
    let parent = dates_of(&records, "n_parent");
    assert_eq!(
        bound(parent.earliest_start.as_ref()),
        Some(date("2026-10-08")),
        "the parent's own work starts after its child's"
    );
}

/// F4: a stage with neither end anchored has no deadline, its contents still held back by
/// today; `closes: false` lifts the close.
#[test]
fn an_unanchored_stage_has_no_deadline() {
    let records = support::journey(&add(&[
        MEETING,
        "{key: n_stage, id: stage, kind: group, title: Stage}",
        "{key: n_work, id: work, parent: n_stage, kind: action, title: Work, estimate: 2}",
        "{key: n_open, id: open, kind: group, title: Open, closes_at: n_meeting, closes: false}",
        "{key: n_inside, id: inside, parent: n_open, kind: action, title: Inside}",
    ]));
    let records = support::accepted(
        &records,
        "- op: set_pin\n  node: n_meeting\n  date: \"2026-10-20\"\n",
    );
    let stage = dates_of(&records, "n_stage");
    assert_eq!(
        (stage.due, stage.latest_start, stage.slack_days),
        (None, None, None)
    );
    assert_eq!(
        bound(stage.earliest_start.as_ref()),
        Some(date("2026-10-08"))
    );
    assert_eq!(dates_of(&records, "n_inside").due, None);
}

/// F4, F6: started work is judged by its finish: its latest start may pass while its
/// finish still fits, with no shortfall; once its due passes, it is overdue and short.
#[test]
fn started_work_is_judged_by_its_finish() {
    let records = support::journey(&add(&[
        MEETING,
        "{key: n_work, id: work, kind: action, title: Work, estimate: 5, due_by: {before: n_meeting}}",
    ]));
    let started = support::accepted(
        &records,
        "- op: transition\n  node: n_work\n  transition: start\n\
- op: set_recorded_date\n  node: n_work\n  end: start\n  date: \"2026-10-01\"\n\
- op: set_pin\n  node: n_meeting\n  date: \"2026-10-07\"\n",
    );
    let derived = support::derived(&started, support::JOURNEY);
    let dates = derived.dates();
    assert_eq!(dates.shortfall_days(&key("n_work")), None);
    assert_eq!(
        dates.slack_days(&key("n_work")),
        Some(1),
        "judged by its due"
    );
    let late = support::accepted(
        &started,
        "- op: set_pin\n  node: n_meeting\n  date: \"2026-10-05\"\n",
    );
    let derived = support::derived(&late, support::JOURNEY);
    assert!(derived.dates().overdue(&key("n_work")));
    assert_eq!(derived.dates().shortfall_days(&key("n_work")), Some(1));
}

/// F6: unstarted work whose latest start is before today is a shortfall and overdue once
/// its due passes; the chain runs from today to the pin.
#[test]
fn work_not_started_in_time_is_short_from_today() {
    let records = support::journey(&add(&[
        MEETING,
        "{key: n_work, id: work, kind: action, title: Work, estimate: 3, due_by: {before: n_meeting}}",
    ]));
    let records = support::accepted(
        &records,
        "- op: set_pin\n  node: n_meeting\n  date: \"2026-10-05\"\n",
    );
    let work = dates_of(&records, "n_work");
    let shortfall = work.shortfall.unwrap();
    assert_eq!(shortfall.shortfall_days, 4);
    assert_eq!(fixers(&shortfall.chain), [FixedBy::Today, FixedBy::Pin]);
    assert!(
        !shortfall.resolutions.is_empty(),
        "a shortfall offers moves"
    );
    assert!(
        support::derived(&records, support::JOURNEY)
            .dates()
            .overdue(&key("n_work"))
    );
}

/// Cycles: two milestones each no earlier than a day before the other are a legal negative
/// cycle, and get bounds from a pin on either.
#[test]
fn a_negative_cycle_gets_bounds() {
    let records = support::journey(&add(&[
        "{key: n_first, id: first, kind: milestone, title: First, not_before: {before: n_second, offset: 1}}",
        "{key: n_second, id: second, kind: milestone, title: Second, not_before: {before: n_first, offset: 1}}",
    ]));
    let records = support::accepted(
        &records,
        "- op: set_pin\n  node: n_first\n  date: \"2026-11-10\"\n",
    );
    let second = dates_of(&records, "n_second");
    assert_eq!(
        bound(second.earliest_start.as_ref()),
        Some(date("2026-11-09"))
    );
    assert_eq!(bound(second.due.as_ref()), Some(date("2026-11-11")));
}

/// F6: two actuals that break a rule between them are a shortfall with the rule on its
/// chain, from one fact to the other; the rule is never dropped.
#[test]
fn two_actuals_that_break_a_rule_are_a_shortfall() {
    let records = support::journey(&add(&[
        "{key: n_freeze, id: freeze, kind: milestone, title: Freeze, due_by: {before: n_beta, offset: 2}}",
        "{key: n_beta, id: beta, kind: milestone, title: Beta}",
    ]));
    let reached = support::accepted(
        &records,
        "- op: transition\n  node: n_freeze\n  transition: reach\n\
- op: transition\n  node: n_beta\n  transition: reach\n\
- op: set_recorded_date\n  node: n_beta\n  end: finish\n  date: \"2026-10-05\"\n",
    );
    let freeze = dates_of(&reached, "n_freeze");
    let shortfall = freeze.shortfall.unwrap();
    assert_eq!(shortfall.shortfall_days, 3);
    assert_eq!(fixers(&shortfall.chain), [FixedBy::Actual, FixedBy::Actual]);
    assert!(matches!(
        shortfall.chain.constraints[..],
        [cairn_schema::Constraint {
            source: ConstraintSource::DueBy { .. },
            ..
        }]
    ));
}

/// F1, F3: today holds back only unfinished decisions, deliverables, and actions. A pending
/// milestone pinned yesterday whose dependency is done is not short; one whose dependency is
/// still open is, by the day today carries to it.
#[test]
fn today_holds_back_only_unfinished_work() {
    let records = support::journey(&add(&[
        "{key: n_work, id: work, kind: action, title: Work}",
        "{key: n_gate, id: gate, kind: milestone, title: Gate, auto_reach: true, requires: [n_work]}",
    ]));
    let pinned = support::accepted(
        &records,
        "- op: set_pin\n  node: n_gate\n  date: \"2026-10-05\"\n",
    );
    let open = support::derived(&pinned, support::JOURNEY);
    assert_eq!(open.dates().shortfall_days(&key("n_gate")), Some(1));
    let done = support::accepted(
        &pinned,
        "- op: transition\n  node: n_work\n  transition: complete\n\
- op: set_recorded_date\n  node: n_work\n  end: finish\n  date: \"2026-10-04\"\n",
    );
    let done = support::derived(&done, support::JOURNEY);
    assert_eq!(done.dates().shortfall_days(&key("n_gate")), None);
    let alone = support::journey(&add(&[
        "{key: n_alone, id: alone, kind: milestone, title: Alone}",
    ]));
    let alone = dates_of(&alone, "n_alone");
    assert_eq!(
        alone.earliest_start, None,
        "a milestone is not held back by today"
    );
}

/// F1, F6: a reached milestone's actual replaces its pin, and a deadline relative to it
/// follows the actual.
#[test]
fn a_deadline_follows_its_milestones_actual() {
    let records = support::journey(&add(&[
        "{key: n_kickoff, id: kickoff, kind: milestone, title: Kickoff}",
        "{key: n_work, id: work, kind: action, title: Work, due_by: {after: n_kickoff, offset: 3}}",
    ]));
    let pinned = support::accepted(
        &records,
        "- op: set_pin\n  node: n_kickoff\n  date: \"2026-10-01\"\n",
    );
    assert_eq!(
        bound(dates_of(&pinned, "n_work").due.as_ref()),
        Some(date("2026-10-04"))
    );
    let reached = support::accepted(
        &pinned,
        "- op: transition\n  node: n_kickoff\n  transition: reach\n",
    );
    let work = dates_of(&reached, "n_work");
    assert_eq!(bound(work.due.as_ref()), Some(date("2026-10-09")));
    let kickoff = dates_of(&reached, "n_kickoff").effective_date.unwrap();
    assert_eq!(
        (kickoff.date, kickoff.origin),
        (date("2026-10-06"), DateOrigin::Actual)
    );
}

/// Pins and actuals bound their own instants: two unrelated pinned chains stay apart, each
/// bound explained by its own pin.
#[test]
fn unrelated_pinned_chains_stay_apart() {
    let records = support::journey(&add(&[
        "{key: n_left, id: left, kind: milestone, title: Left}",
        "{key: n_right, id: right, kind: milestone, title: Right}",
        "{key: n_left_work, id: left-work, kind: action, title: Left work, due_by: {before: n_left}}",
        "{key: n_right_work, id: right-work, kind: action, title: Right work, due_by: {before: n_right}}",
    ]));
    let records = support::accepted(
        &records,
        "- op: set_pin\n  node: n_left\n  date: \"2026-11-01\"\n\
- op: set_pin\n  node: n_right\n  date: \"2026-12-01\"\n",
    );
    for (work, milestone, due) in [
        ("n_left_work", "n_left", "2026-11-01"),
        ("n_right_work", "n_right", "2026-12-01"),
    ] {
        let found = dates_of(&records, work).due.unwrap();
        assert_eq!(found.date, date(due));
        let pins: Vec<_> = found
            .chain
            .fixed
            .iter()
            .map(|f| f.instant.clone())
            .collect();
        let expected = Instant::Node {
            node: key(milestone),
            point: InstantPoint::Finish,
        };
        assert_eq!(pins, [expected]);
    }
}

/// F2: constraints from not-relevant nodes are dropped; through undecided nodes they are
/// kept and marked conditional; skipped work takes no time.
#[test]
fn relevance_and_skip_shape_the_network() {
    let records = support::journey(&add(&[
        MEETING,
        "{key: n_flag, id: flag, kind: decision, title: Flag, prompt: Flag?, answer_type: boolean}",
        "{key: n_maybe, id: maybe, kind: action, title: Maybe, estimate: 5, relevant_when: {equals: {decision: n_flag, value: true}}, due_by: {before: n_meeting}}",
        "{key: n_after, id: after, kind: action, title: After, requires: [n_maybe]}",
    ]));
    let records = support::accepted(
        &records,
        "- op: set_pin\n  node: n_meeting\n  date: \"2026-11-20\"\n",
    );
    let maybe = dates_of(&records, "n_maybe").latest_start.unwrap();
    assert!(
        maybe.chain.constraints.iter().all(|c| c.conditional),
        "{maybe:#?}"
    );
    let after = dates_of(&records, "n_after");
    assert_eq!(
        bound(after.earliest_start.as_ref()),
        Some(date("2026-10-11"))
    );
    let skipped = support::accepted(
        &records,
        "- op: transition\n  node: n_maybe\n  transition: {skip: {reason: Not needed.}}\n",
    );
    let after = dates_of(&skipped, "n_after");
    assert_eq!(
        bound(after.earliest_start.as_ref()),
        Some(date("2026-10-06"))
    );
    let not_relevant = support::accepted(
        &records,
        "- op: answer\n  decision: n_flag\n  value: {boolean: false}\n",
    );
    assert_eq!(dates_of(&not_relevant, "n_maybe"), NodeDates::default());
}

// Review round 1 regressions.

/// F6: each move a shortfall lists resolves it applied alone, so none moves a pin that today
/// or an actual already overrides.
#[test]
fn every_shortfall_move_resolves_it() {
    let records = support::journey(&add(&[
        "{key: n_choice, id: choice, kind: decision, title: Choice, prompt: Which?, answer_type: text}",
    ]));
    let cases = [
        (
            support::accepted(
                &records,
                "- op: set_pin\n  node: n_choice\n  date: \"2026-10-05\"\n",
            ),
            support::JOURNEY,
            "n_choice",
        ),
        (
            support::finished("product-launch"),
            "j_launch",
            "n_code_freeze",
        ),
        // Review round 3: today, through the work's start, still holds its pinned finish.
        (
            support::accepted(
                &support::journey(&add(&[
                    "{key: n_work, id: work, kind: action, title: Work, estimate: 3}",
                    "{key: n_after, id: after, kind: milestone, title: After, requires: [n_work]}",
                ])),
                "- op: set_pin\n  node: n_work\n  date: \"2026-10-10\"\n\
- op: transition\n  node: n_after\n  transition: reach\n\
- op: apply_override\n  node: n_after\n  override: {guard_bypass: {guards: [deps_done], reason: Reached ahead of the work.}}\n",
            ),
            support::JOURNEY,
            "n_work",
        ),
    ];
    for (records, journey, node) in cases {
        let derived = support::derived(&records, journey);
        let graph = support::journey_graph(&records, journey);
        let shortfall = derived.dates().node(&graph, &key(node)).shortfall.unwrap();
        assert!(!shortfall.resolutions.is_empty(), "{node} offers moves");
        for resolution in &shortfall.resolutions {
            let resolution = cairn_schema::to_json(resolution).unwrap();
            let moved = support::accepted_on(&records, journey, &format!("- {resolution}\n"));
            let after = support::derived(&moved, journey);
            assert_eq!(
                after.dates().shortfall_days(&key(node)),
                None,
                "{node}: {resolution}"
            );
        }
    }
}

// Review round 2 regression.

/// F6: a fact earlier than a rule from `created_at` or a date answer allows is short, shown
/// on the node whose fact it is, since neither source is a node of its own.
#[test]
fn a_fact_too_early_for_created_at_or_an_answer_is_short() {
    let records = support::journey(&add(&[
        "{key: n_date, id: date, kind: decision, title: Date, prompt: When?, answer_type: date}",
        "{key: n_settled, id: settled, kind: milestone, title: Settled, not_before: {after: journey.created_at, offset: 3}}",
        "{key: n_answered, id: answered, kind: milestone, title: Answered, not_before: {after: n_date}}",
    ]));
    let reached = support::accepted(
        &records,
        "- op: answer\n  decision: n_date\n  value: {date: \"2026-10-10\"}\n\
- op: transition\n  node: n_settled\n  transition: reach\n\
- op: transition\n  node: n_answered\n  transition: reach\n",
    );
    for (node, days, source) in [
        ("n_settled", 3, Instant::CreatedAt),
        (
            "n_answered",
            4,
            Instant::Answer {
                decision: key("n_date"),
            },
        ),
    ] {
        let shortfall = dates_of(&reached, node).shortfall.unwrap();
        assert_eq!(shortfall.shortfall_days, days, "{node}");
        assert_eq!(shortfall.chain.fixed[0].instant, source, "{node}");
    }
}
