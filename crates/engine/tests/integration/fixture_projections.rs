//! C2, C4, C10 on every fixture at the scenario point fixtures/README.md states: the visible
//! set with actions hidden (each node and the nearest visible ancestor it is drawn in), what
//! rolls up into each visible node, and the first three items of the next list, read at
//! 2026-10-06.
#![cfg(test)]

use crate::support;

use std::collections::{BTreeMap, BTreeSet};

use cairn_engine::DerivedJourney;
use cairn_schema::{NextQuery, NodeKind};

struct Expected {
    fixture: &'static str,
    journey: &'static str,
    steps: usize,
    /// Each visible node drawn inside another, as (node, parent); the rest are at the top.
    nested: &'static [(&'static str, &'static str)],
    top: &'static [&'static str],
    rolled_up: &'static [(&'static str, &'static [&'static str])],
    next: &'static [&'static str],
}

const EXPECTED: [Expected; 4] = [
    Expected {
        fixture: "vendor-evaluation",
        journey: "j_vendor_eval",
        steps: 3,
        nested: &[
            ("n_final_review", "n_reporting"),
            ("n_final_report", "n_final_review"),
            ("n_findings", "n_reporting"),
            ("n_findings_reviewer", "n_reporting"),
            ("n_review_opens", "n_reporting"),
            ("n_access", "n_setup"),
            ("n_plan", "n_setup"),
            ("n_workload", "n_setup"),
            ("n_baseline", "n_testing"),
            ("n_comparison_set", "n_testing"),
            ("n_partner_led", "n_testing"),
        ],
        top: &[
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
        ],
        rolled_up: &[
            ("n_partner_led", &["n_criteria", "n_partner_results"]),
            ("n_plan", &["n_plan_draft", "n_plan_review"]),
        ],
        next: &["n_access", "n_decision_meeting", "n_workload"],
    },
    Expected {
        fixture: "hiring-loop",
        journey: "j_hiring",
        steps: 6,
        nested: &[("n_onsite", "n_loop"), ("n_interviews", "n_onsite")],
        top: &["n_choose_panel", "n_loop", "n_make_offer", "n_offer"],
        rolled_up: &[
            (
                "n_interviews",
                &["n_interview_one", "n_interview_three", "n_interview_two"],
            ),
            ("n_loop", &["n_screen"]),
            (
                "n_onsite",
                &["n_debrief", "n_debrief_notes", "n_debrief_scorecard"],
            ),
        ],
        next: &["n_offer"],
    },
    Expected {
        fixture: "product-launch",
        journey: "j_launch",
        steps: 4,
        nested: &[
            ("n_features", "n_build"),
            ("n_announcement", "n_materials"),
            ("n_docs", "n_materials"),
        ],
        top: &[
            "n_beta",
            "n_beta_end",
            "n_beta_start",
            "n_build",
            "n_code_freeze",
            "n_kickoff",
            "n_launch",
            "n_materials",
            "n_retro",
        ],
        rolled_up: &[
            ("n_beta", &["n_beta_feedback"]),
            ("n_build", &["n_hardening"]),
        ],
        next: &["n_launch", "n_docs", "n_announcement"],
    },
    Expected {
        fixture: "bake-off",
        journey: "j_bakeoff",
        steps: 4,
        nested: &[],
        top: &[
            "n_comparison",
            "n_criteria",
            "n_judges",
            "n_summary",
            "n_trial_a",
            "n_trial_b",
            "n_winner",
            "n_wrap_up",
        ],
        rolled_up: &[],
        next: &["n_trial_a", "n_trial_b", "n_wrap_up"],
    },
];

#[test]
fn each_fixture_hides_its_actions_and_lists_what_is_next() {
    let shown: BTreeSet<NodeKind> = NodeKind::ALL
        .into_iter()
        .filter(|kind| *kind != NodeKind::Action)
        .collect();
    for expected in &EXPECTED {
        let records = support::after(expected.fixture, expected.steps);
        let graph = support::journey_graph(&records, expected.journey);
        let derived = support::derived(&records, expected.journey);
        let journey = DerivedJourney::new(&graph, &derived);
        let level = journey.level(&shown, None).unwrap();
        let placed: BTreeMap<&str, Option<&str>> = level
            .nodes
            .iter()
            .map(|node| {
                (
                    node.key.as_str(),
                    node.parent.as_ref().map(cairn_schema::NodeKey::as_str),
                )
            })
            .collect();
        let mut wanted: BTreeMap<&str, Option<&str>> =
            expected.top.iter().map(|key| (*key, None)).collect();
        wanted.extend(
            expected
                .nested
                .iter()
                .map(|(key, parent)| (*key, Some(*parent))),
        );
        assert_eq!(placed, wanted, "{}", expected.fixture);
        let rolled: Vec<(&str, Vec<&str>)> = level
            .nodes
            .iter()
            .filter(|node| !node.rolled_up.is_empty())
            .map(|node| {
                (
                    node.key.as_str(),
                    node.rolled_up
                        .iter()
                        .map(cairn_schema::NodeKey::as_str)
                        .collect(),
                )
            })
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .collect();
        let wanted_rolled: Vec<(&str, Vec<&str>)> = expected
            .rolled_up
            .iter()
            .map(|(key, held)| (*key, held.to_vec()))
            .collect();
        assert_eq!(rolled, wanted_rolled, "{}", expected.fixture);
        let next = journey
            .next(&NextQuery::default(), &BTreeSet::new())
            .unwrap();
        let first: Vec<&str> = next
            .items
            .iter()
            .take(3)
            .map(|row| row.key.as_str())
            .collect();
        assert_eq!(first, expected.next, "{}", expected.fixture);
    }
}
