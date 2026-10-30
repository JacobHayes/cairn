//! The execution layer (A8, F1 to F7; ARCHITECTURE, Date network) at the fixed clock (today
//! is 2026-10-06; the test journey was created that day): bounds both ways from pins,
//! actuals, and today, each with its chain; effective dates; slack; `overdue`; and
//! `shortfall` with its chain, never a rejection.
#![cfg(test)]

use crate::engine::support;

use cairn_engine::Records;
use cairn_schema::{
    ConstraintSource, Date, DateOrigin, FixedBy, Instant, InstantPoint, NodeDates, Rejection,
    ViolationCode,
};
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

fn due_day(found: Option<&cairn_schema::DueBound>) -> Option<Date> {
    found.map(|due| due.date.day())
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
    assert_eq!(due_day(report.due.as_ref()), Some(date("2026-11-06")));
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
        due_day(dates_of(&records, "n_both").due.as_ref()),
        Some(date("2026-11-01"))
    );
    let after = dates_of(&records, "n_after");
    assert_eq!(
        bound(after.earliest_start.as_ref()),
        Some(date("2026-11-04"))
    );
    let soon = dates_of(&records, "n_soon");
    assert_eq!(due_day(soon.due.as_ref()), Some(date("2026-10-09")));
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
    assert_eq!(due_day(report.due.as_ref()), Some(date("2026-11-20")));
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
            ConstraintSource::DueBy {
                node: key("n_final_review")
            },
        ]
    );
    let end = latest.chain.fixed.last().unwrap();
    assert_eq!(end.date.day(), date("2026-11-20"));
    assert_eq!(end.fixed_by, FixedBy::Pin);
    let meeting = derived
        .dates()
        .effective_date(&key("n_decision_meeting"))
        .unwrap();
    assert_eq!(
        (meeting.date, meeting.origin),
        (date("2026-11-20").into(), DateOrigin::Pin)
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
    assert_eq!(due_day(child.due.as_ref()), Some(date("2026-11-06")));
    assert_eq!(bound(child.latest_start.as_ref()), Some(date("2026-11-04")));
    let parent = dates_of(&records, "n_parent");
    assert_eq!(
        bound(parent.earliest_start.as_ref()),
        Some(date("2026-10-08")),
        "the parent's own work starts after its child's"
    );
}

/// F4: a stage with neither end anchored has no deadline, its contents still held back by
/// today.
#[test]
fn an_unanchored_stage_has_no_deadline() {
    let records = support::journey(&add(&[
        MEETING,
        "{key: n_stage, id: stage, kind: group, title: Stage}",
        "{key: n_work, id: work, parent: n_stage, kind: action, title: Work, estimate: 2}",
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
    assert_eq!(due_day(second.due.as_ref()), Some(date("2026-11-11")));
}

/// F6: an actual that breaks a rule of work still to do is that work's shortfall, with the
/// rule on its chain from the fact to the pin; once the work is done too, the two facts break
/// it between them and nothing is short.
#[test]
fn an_actual_that_breaks_a_rule_of_remaining_work_is_its_shortfall() {
    let records = support::journey(&add(&[
        "{key: n_freeze, id: freeze, kind: milestone, title: Freeze}",
        "{key: n_beta, id: beta, kind: milestone, title: Beta, not_before: {after: n_freeze, offset: 2}}",
    ]));
    let frozen = support::accepted(
        &records,
        "- op: set_pin\n  node: n_beta\n  date: \"2026-10-07\"\n\
- op: transition\n  node: n_freeze\n  transition: complete\n",
    );
    let shortfall = dates_of(&frozen, "n_beta").shortfall.unwrap();
    assert_eq!(shortfall.shortfall_days, 1);
    assert_eq!(fixers(&shortfall.chain), [FixedBy::Actual, FixedBy::Pin]);
    assert!(matches!(
        shortfall.chain.constraints[..],
        [cairn_schema::Constraint {
            source: ConstraintSource::NotBefore { .. },
            ..
        }]
    ));
    assert_eq!(dates_of(&frozen, "n_freeze").shortfall, None);
    let both = support::accepted(
        &frozen,
        "- op: transition\n  node: n_beta\n  transition: complete\n",
    );
    for node in ["n_freeze", "n_beta"] {
        assert_eq!(dates_of(&both, node).shortfall, None, "{node}");
    }
}

/// F1, F3: today holds back only unfinished decisions, deliverables, and actions. A pending
/// milestone pinned yesterday whose dependency is done is not short; one whose dependency is
/// still open is, by the day today carries to it.
#[test]
fn today_holds_back_only_unfinished_work() {
    let records = support::journey(&add(&[
        "{key: n_work, id: work, kind: action, title: Work}",
        "{key: n_gate, id: gate, kind: milestone, title: Gate, auto_done: true, requires: [n_work]}",
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

/// F1, F6: a done milestone's actual replaces its pin, and a deadline relative to it
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
        due_day(dates_of(&pinned, "n_work").due.as_ref()),
        Some(date("2026-10-04"))
    );
    let finished = support::accepted(
        &pinned,
        "- op: transition\n  node: n_kickoff\n  transition: complete\n",
    );
    let work = dates_of(&finished, "n_work");
    assert_eq!(due_day(work.due.as_ref()), Some(date("2026-10-09")));
    let kickoff = dates_of(&finished, "n_kickoff").effective_date.unwrap();
    assert_eq!(
        (kickoff.date, kickoff.origin),
        (date("2026-10-06").into(), DateOrigin::Actual)
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
        assert_eq!(found.date.day(), date(due));
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

/// F1, F3: a skipped milestone's pin, and the date answered for it, stop counting: whatever
/// is due before it, a group or any other node, has no deadline from it, and gets it back when
/// the milestone is reopened.
#[test]
fn a_skipped_milestones_dates_stop_counting() {
    let nodes = [
        "{key: n_stage, id: stage, kind: group, title: Stage, due_by: {before: n_meeting}}",
        "{key: n_in, id: in, parent: n_stage, kind: action, title: In, estimate: 2}",
        "{key: n_work, id: work, kind: action, title: Work, estimate: 2, due_by: {before: n_meeting}}",
    ];
    let pinned = support::journey(&add(&[&[MEETING], &nodes[..]].concat()));
    let pinned = support::accepted(
        &pinned,
        "- op: set_pin\n  node: n_meeting\n  date: \"2026-11-20\"\n",
    );
    let fed = "{key: n_when, id: when, kind: decision, title: When, prompt: When?, answer_type: date, feeds_milestone: n_meeting}";
    let answered = support::journey(&add(&[&[MEETING, fed], &nodes[..]].concat()));
    let answered = support::accepted(
        &answered,
        "- op: answer\n  decision: n_when\n  value: {date: \"2026-11-25\"}\n",
    );
    let due = |records: &Records, node: &str| due_day(dates_of(records, node).due.as_ref());
    for (records, expected) in [(&pinned, "2026-11-20"), (&answered, "2026-11-25")] {
        assert_eq!(due(records, "n_stage"), Some(date(expected)));
        assert_eq!(due(records, "n_work"), Some(date(expected)));
        let skipped = support::accepted(
            records,
            "- op: transition\n  node: n_meeting\n  transition: {skip: {reason: Not needed.}}\n",
        );
        assert_eq!(
            due(&skipped, "n_stage"),
            None,
            "a skipped milestone sets no deadline"
        );
        assert_eq!(due(&skipped, "n_work"), None);
        assert_eq!(due(&skipped, "n_in"), None);
        let reopened = support::accepted(
            &skipped,
            "- op: transition\n  node: n_meeting\n  transition: reopen\n",
        );
        assert_eq!(due(&reopened, "n_stage"), Some(date(expected)));
        assert_eq!(due(&reopened, "n_work"), Some(date(expected)));
    }
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
        // Work still to do that a late actual pushes past its deadline.
        (
            support::accepted(
                &support::journey(&add(&[
                    "{key: n_kickoff, id: kickoff, kind: milestone, title: Kickoff}",
                    "{key: n_review, id: review, kind: milestone, title: Review}",
                    "{key: n_draft, id: draft, kind: action, title: Draft, estimate: 1, not_before: {after: n_kickoff, offset: 5}, due_by: {before: n_review}}",
                ])),
                "- op: set_pin\n  node: n_review\n  date: \"2026-10-10\"\n\
- op: transition\n  node: n_kickoff\n  transition: complete\n\
- op: set_recorded_date\n  node: n_kickoff\n  end: finish\n  date: \"2026-10-05\"\n",
            ),
            support::JOURNEY,
            "n_draft",
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

/// F6: a fact earlier than its own rule from `created_at` or a date answer allows is a fact,
/// not a shortfall, for a start and a finish alike; and a code freeze finished after the
/// launch's chain allowed is not short, nor the launch, and the journey takes the next patch.
#[test]
fn a_fact_against_its_own_rule_is_never_short() {
    let records = support::journey(&add(&[
        "{key: n_date, id: date, kind: decision, title: Date, prompt: When?, answer_type: date}",
        "{key: n_settled, id: settled, kind: action, title: Settled, not_before: {after: journey.created_at}}",
        "{key: n_answered, id: answered, kind: action, title: Answered, not_before: {after: n_date}}",
    ]));
    let finished = support::accepted(
        &records,
        "- op: answer\n  decision: n_date\n  value: {date: \"2026-10-05\"}\n\
- op: transition\n  node: n_settled\n  transition: start\n\
- op: transition\n  node: n_settled\n  transition: complete\n\
- op: transition\n  node: n_answered\n  transition: start\n\
- op: transition\n  node: n_answered\n  transition: complete\n\
- op: set_recorded_date\n  node: n_settled\n  end: start\n  date: \"2026-10-01\"\n\
- op: set_recorded_date\n  node: n_settled\n  end: finish\n  date: \"2026-10-04\"\n\
- op: set_recorded_date\n  node: n_answered\n  end: start\n  date: \"2026-10-01\"\n\
- op: set_recorded_date\n  node: n_answered\n  end: finish\n  date: \"2026-10-02\"\n",
    );
    for node in ["n_settled", "n_answered"] {
        assert_eq!(dates_of(&finished, node).shortfall, None, "{node}");
    }
    let launch = support::finished("product-launch");
    let derived = support::derived(&launch, "j_launch");
    for node in ["n_code_freeze", "n_launch"] {
        assert_eq!(derived.dates().shortfall_days(&key(node)), None, "{node}");
    }
    support::accepted_on(
        &launch,
        "j_launch",
        "- op: add_annotation\n  annotation: {key: a_late, node: n_code_freeze, note: Slipped two days.}\n",
    );
}

/// F4, F5: a stage due five days after it opens holds two three-day pieces of work
/// side by side, but not one after the other.
#[test]
fn a_stage_window_holds_parallel_estimates_but_not_sequential_ones() {
    let records = support::journey(&add(&[
        "{key: n_opens, id: opens, kind: milestone, title: Opens}",
        "{key: n_closes, id: closes, kind: milestone, title: Closes, due_by: {after: n_opens, offset: 5}}",
        "{key: n_window, id: window, kind: group, title: Window, requires: [n_opens], due_by: {before: n_closes}}",
        "{key: n_first, id: first, parent: n_window, kind: action, title: First, estimate: 3}",
        "{key: n_second, id: second, parent: n_window, kind: action, title: Second, estimate: 3}",
    ]));
    let sequential = support::journey_patch(
        &records,
        "- op: add_edge\n  edge: {node: n_second, requires: n_first}\n",
    );
    let Err(Rejection::Invalid { violations }) = sequential else {
        panic!("sequential work past the window is invalid");
    };
    let found = &violations.as_slice()[0];
    assert_eq!(found.code, ViolationCode::ContradictoryChain);
    assert_eq!(
        found.chains.as_ref().unwrap().chains.as_slice()[0].shortfall_days,
        1
    );
    let pinned = support::accepted(
        &records,
        "- op: set_pin\n  node: n_opens\n  date: \"2026-11-02\"\n",
    );
    assert_eq!(
        due_day(dates_of(&pinned, "n_second").due.as_ref()),
        Some(date("2026-11-07")),
        "parallel work fits the window"
    );
}
