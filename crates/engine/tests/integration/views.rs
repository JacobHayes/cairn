//! C12's decision view, C13's timeline, C18's status summary, and A10/G3's message drafts, on
//! the fixtures at 2026-10-06.
#![cfg(test)]

use crate::support;

use cairn_engine::{Derived, DerivedJourney, DraftContext, Graph, Records};
use cairn_schema::{
    AnswerValue, DateOrigin, Journey, KeyRefs, MessageTemplate, NodeKey, ResourceContent, State,
};
use support::key;

const VENDOR: &str = "j_vendor_eval";

struct Derive {
    records: Records,
    graph: Graph,
    derived: Derived,
}

impl Derive {
    fn of(records: Records, journey: &str) -> Self {
        Self {
            graph: support::journey_graph(&records, journey),
            derived: support::derived(&records, journey),
            records,
        }
    }

    fn vendor(steps: usize) -> Self {
        Self::of(support::vendor_after(steps), VENDOR)
    }

    fn journey(&self) -> DerivedJourney<'_> {
        DerivedJourney::new(&self.graph, &self.derived)
    }

    fn header(&self) -> &Journey {
        &self.records.journeys[&VENDOR.parse().unwrap()]
    }
}

fn names(keys: &[NodeKey]) -> Vec<&str> {
    keys.iter().map(NodeKey::as_str).collect()
}

/// C12: every decision, in tree order, with what its answer affects: the partner decision
/// decides the partner-led subset's relevance and the comparison set the baseline's; the date
/// pins the meeting; the entity answers fill roles. A decision gated by work that is hidden
/// carries the marker.
#[test]
fn the_decision_view_shows_what_each_answer_affects() {
    let view = Derive::vendor(1).journey().decision_view();
    let keys: Vec<&str> = view
        .decisions
        .iter()
        .map(|entry| entry.node.as_str())
        .collect();
    assert_eq!(
        keys,
        [
            "n_meeting_date",
            "n_partner_runs",
            "n_purpose",
            "n_findings_reviewer",
            "n_comparison_set",
            "n_who_informed",
            "n_who_owns",
        ]
    );
    let entry = |name: &str| {
        view.decisions
            .iter()
            .find(|entry| entry.node.as_str() == name)
            .unwrap()
    };
    assert_eq!(
        names(&entry("n_partner_runs").affects),
        ["n_criteria", "n_partner_led", "n_partner_results"]
    );
    assert_eq!(names(&entry("n_comparison_set").affects), ["n_baseline"]);
    assert_eq!(
        entry("n_meeting_date").pins,
        Some(key("n_decision_meeting"))
    );
    assert_eq!(
        entry("n_who_owns").fills,
        Some("r_eval_owner".parse().unwrap())
    );
    assert_eq!(
        names(&entry("n_comparison_set").hidden_prerequisites),
        ["n_plan"]
    );
    assert_eq!(view.edges.len(), 0, "no decision requires another");
    let decided = Derive::vendor(2).journey().decision_view();
    let purpose = decided
        .decisions
        .iter()
        .find(|entry| entry.node.as_str() == "n_purpose");
    let purpose = purpose.unwrap();
    assert_eq!(purpose.state, State::Decided);
    assert_eq!(
        purpose.answer,
        Some(AnswerValue::SingleChoice("purchase".parse().unwrap()))
    );
}

/// C13: before the meeting date is decided nothing reaches the milestones, so they are
/// undated; once it is, the meeting is the end anchor at its pin, kickoff and the review
/// opening sit at their due dates, and open work at its due date. A pin on the final report
/// shows as a pin.
#[test]
fn the_timeline_places_milestones_pins_and_due_dates() {
    let created = Derive::vendor(1).journey().timeline();
    assert_eq!(
        names(&created.undated),
        ["n_decision_meeting", "n_kickoff", "n_review_opens"]
    );
    assert_eq!(created.end, Some(key("n_decision_meeting")));
    let decided = Derive::vendor(2).journey().timeline();
    assert_eq!(decided.undated.len(), 0);
    let at = |timeline: &cairn_schema::Timeline, name: &str| {
        let entry = timeline
            .entries
            .iter()
            .find(|entry| entry.node.as_str() == name);
        let entry = entry
            .unwrap_or_else(|| panic!("{name} is on the timeline"))
            .clone();
        (entry.date.to_string(), entry.origin, entry.is_final)
    };
    assert_eq!(
        at(&decided, "n_decision_meeting"),
        ("2026-11-20".to_owned(), DateOrigin::Pin, true)
    );
    assert_eq!(
        at(&decided, "n_kickoff"),
        ("2026-10-29".to_owned(), DateOrigin::Due, false)
    );
    assert_eq!(
        at(&decided, "n_access"),
        ("2026-10-31".to_owned(), DateOrigin::Due, false)
    );
    assert!(
        decided
            .entries
            .windows(2)
            .all(|pair| pair[0].date <= pair[1].date)
    );
    let pinned = Derive::vendor(7).journey().timeline();
    assert_eq!(
        at(&pinned, "n_final_report"),
        ("2026-11-02".to_owned(), DateOrigin::Pin, false)
    );
    assert!(
        !pinned
            .entries
            .iter()
            .any(|entry| entry.node.as_str() == "n_access"),
        "done work has no due date to show"
    );
}

/// C18: after kickoff, the summary counts in-scope nodes by state, lists the milestones still
/// ahead by date, and the open decisions with their owner; on the product launch, its
/// shortfalls.
#[test]
fn the_status_summary_counts_and_lists() {
    let summary = Derive::vendor(3).journey().status_summary();
    assert_eq!(summary.by_state.get(&State::Reached), Some(&1));
    assert_eq!(summary.by_state.get(&State::Decided), Some(&5));
    assert_eq!(
        summary.remaining,
        summary.by_state.values().sum::<u32>() - 6
    );
    let upcoming: Vec<(&str, String)> = summary
        .upcoming_milestones
        .iter()
        .map(|milestone| (milestone.node.as_str(), milestone.date.date.to_string()))
        .collect();
    assert_eq!(
        upcoming,
        [
            ("n_review_opens", "2026-11-06".to_owned()),
            ("n_decision_meeting", "2026-11-20".to_owned())
        ]
    );
    let open: Vec<(&str, Vec<&str>)> = summary
        .open_decisions
        .iter()
        .map(|decision| {
            let owners = decision
                .owners
                .iter()
                .map(cairn_schema::EntityKey::as_str)
                .collect();
            (decision.node.as_str(), owners)
        })
        .collect();
    assert_eq!(
        open,
        [
            ("n_comparison_set", vec!["e_lead"]),
            ("n_findings_reviewer", vec!["e_lead"])
        ]
    );
    assert_eq!((summary.overdue.len(), summary.stale.len()), (0, 0));
    let launch = Derive::of(support::finished("product-launch"), "j_launch");
    assert_eq!(
        names(&launch.journey().status_summary().shortfalls),
        ["n_code_freeze", "n_launch"]
    );
}

/// The access request drafted on environment access.
fn access_request(derive: &Derive) -> MessageTemplate<KeyRefs> {
    let node = derive.graph.node(&key("n_access")).unwrap();
    match &node.resources[0].content {
        ResourceContent::MessageDraft(template) => template.clone(),
        other => panic!("{other:?}"),
    }
}

/// A10, G3: before the owner and purpose are decided, the access request renders with a
/// marker for each; once they are, it reads in full. Rendering never fails.
#[test]
fn a_message_draft_marks_what_it_cannot_fill() {
    for (steps, expected, complete) in [
        (
            1,
            "Hello, [missing: roles.eval_owner.name] needs access to the evaluation environment for Evaluate the analytics vendor. The purpose is: [missing: answers.purpose].",
            false,
        ),
        (
            2,
            "Hello, Evaluation Lead needs access to the evaluation environment for Evaluate the analytics vendor. The purpose is: purchase.",
            true,
        ),
    ] {
        let derive = Derive::vendor(steps);
        let context = DraftContext {
            header: &derive.header().header,
            url: None,
            deployment: &derive.records.deployment,
        };
        let rendered = derive
            .journey()
            .render_draft(&access_request(&derive), &context);
        assert_eq!(
            (rendered.text().as_str(), rendered.is_complete()),
            (expected, complete)
        );
    }
}

/// A10: journey fields render from the header and the host's link, which is a marker when the
/// host gives none.
#[test]
fn journey_fields_render_from_the_header_and_link() {
    let derive = Derive::vendor(1);
    let template: MessageTemplate<KeyRefs> =
        "{{journey.status}} since {{journey.created_at}}: {{journey.url}}"
            .parse()
            .unwrap();
    let url = "https://example.org/journeys/vendor".parse().unwrap();
    for (link, expected) in [
        (None, "active since 2026-10-01: [missing: journey.url]"),
        (
            Some(&url),
            "active since 2026-10-01: https://example.org/journeys/vendor",
        ),
    ] {
        let context = DraftContext {
            header: &derive.header().header,
            url: link,
            deployment: &derive.records.deployment,
        };
        assert_eq!(
            derive.journey().render_draft(&template, &context).text(),
            expected
        );
    }
}

// Review round 1 regressions.

/// C12: a decision affects every node a condition reading it can rule out: the group it
/// conditions and the group's child, even where the child's own condition reads another
/// decision; a force include beneath drops the condition, so that node is not affected.
#[test]
fn a_decision_affects_what_its_condition_holds_beneath_it() {
    let records = support::journey(&support::add_nodes(&[
        "{key: n_a, id: a, kind: decision, title: A, prompt: A?, answer_type: boolean}",
        "{key: n_b, id: b, kind: decision, title: B, prompt: B?, answer_type: boolean}",
        "{key: n_group, id: group, kind: group, title: Group, relevant_when: {equals: {decision: n_a, value: true}}}",
        "{key: n_child, id: child, parent: n_group, kind: action, title: Child, relevant_when: {equals: {decision: n_b, value: true}}}",
        "{key: n_kept, id: kept, parent: n_group, kind: action, title: Kept}",
    ]));
    let records = support::accepted(
        &records,
        "- op: answer\n  decision: n_a\n  value: {boolean: true}\n- op: answer\n  decision: n_b\n  value: {boolean: true}\n- op: apply_override\n  node: n_kept\n  override: {force_include: {reason: Always needed.}}\n",
    );
    let derive = Derive::of(records, support::JOURNEY);
    let view = derive.journey().decision_view();
    let affects = |name: &str| {
        let entry = view
            .decisions
            .iter()
            .find(|entry| entry.node.as_str() == name);
        names(&entry.unwrap().affects).join(" ")
    };
    assert_eq!(affects("n_a"), "n_child n_group");
    assert_eq!(affects("n_b"), "n_child");
}

/// A10, E6: an entity answer given before a merge renders the survivor's name.
#[test]
fn a_merged_entity_answer_renders_the_survivors_name() {
    let derive = Derive::of(support::vendor_lead_merged(), VENDOR);
    let template: MessageTemplate<KeyRefs> = "Owner: {{answers.n_who_owns}}".parse().unwrap();
    let context = DraftContext {
        header: &derive.header().header,
        url: None,
        deployment: &derive.records.deployment,
    };
    let rendered = derive.journey().render_draft(&template, &context);
    assert_eq!(rendered.text(), "Owner: Other");
}

// Review round 2 regressions.

/// C12: a decision affects what it rules out through another decision's relevance: a
/// condition reads a decision that is not relevant as unanswered.
#[test]
fn a_decision_affects_what_the_decisions_it_rules_out_affect() {
    let chained = |first: bool| {
        let records = support::journey(&support::add_nodes(&[
            "{key: n_d1, id: d1, kind: decision, title: D1, prompt: D1?, answer_type: boolean}",
            "{key: n_d2, id: d2, kind: decision, title: D2, prompt: D2?, answer_type: boolean, relevant_when: {equals: {decision: n_d1, value: true}}}",
            "{key: n_x, id: x, kind: action, title: X, relevant_when: {equals: {decision: n_d2, value: true}}}",
        ]));
        let answers = format!(
            "- op: answer\n  decision: n_d1\n  value: {{boolean: {first}}}\n{}",
            if first {
                "- op: answer\n  decision: n_d2\n  value: {boolean: true}\n"
            } else {
                ""
            }
        );
        Derive::of(support::accepted(&records, &answers), support::JOURNEY)
    };
    let (yes, no) = (chained(true), chained(false));
    let x = key("n_x");
    assert_ne!(
        yes.derived.relevance().value(&x),
        no.derived.relevance().value(&x)
    );
    let view = yes.journey().decision_view();
    let d1 = view
        .decisions
        .iter()
        .find(|entry| entry.node.as_str() == "n_d1")
        .unwrap();
    assert_eq!(names(&d1.affects), ["n_d2", "n_x"]);
}

/// C18, I3: a decision under a skipped container is closed (D1a), so neither the summary nor
/// the snapshot lists it as open.
#[test]
fn an_effectively_skipped_decision_is_not_open() {
    let records = support::journey(&support::add_nodes(&[
        "{key: n_box, id: box, kind: group, title: Box}",
        "{key: n_d, id: d, parent: n_box, kind: decision, title: D, prompt: D?, answer_type: boolean}",
        "{key: n_a, id: a, kind: action, title: A}",
    ]));
    let records = support::accepted(
        &records,
        "- op: transition\n  node: n_box\n  transition: {skip: {reason: Not needed.}}\n",
    );
    let derive = Derive::of(records, support::JOURNEY);
    let journey = derive.journey();
    assert!(derive.derived.blocking().closed(&key("n_d")));
    assert_eq!(journey.status_summary().open_decisions.len(), 0);
    let snapshot = journey
        .snapshot(&cairn_schema::SnapshotScope::default())
        .unwrap();
    assert_eq!(snapshot.open_decisions.len(), 0);
}

/// I3, C12, E6: an entity answer given before a merge names the survivor in the snapshot and
/// the decision view, as owners do.
#[test]
fn a_merged_entity_answer_names_the_survivor_in_snapshot_and_decision_view() {
    let derive = Derive::of(support::vendor_lead_merged(), VENDOR);
    let journey = derive.journey();
    let who = key("n_who_owns");
    let survivor = Some(AnswerValue::Entity("e_other".parse().unwrap()));
    let view = journey.decision_view();
    let entry = view
        .decisions
        .iter()
        .find(|entry| entry.node == who)
        .unwrap();
    assert_eq!(entry.answer, survivor);
    let snapshot = journey
        .snapshot(&cairn_schema::SnapshotScope::default())
        .unwrap();
    assert_eq!(snapshot.answers.get(&who).cloned(), survivor);
}
