//! C7, trace: upstream and downstream across levels over the full structural graph, terminal
//! and not-relevant nodes included, with gravity's contributors marked, on the vendor
//! evaluation at 2026-10-06.
#![cfg(test)]

use crate::engine::support;

use cairn_engine::{DerivedJourney, ProjectionError, Records};
use cairn_schema::{NodeKey, Trace};
use support::key;

const VENDOR: &str = "j_vendor_eval";

fn trace(records: &Records, node: &str) -> Trace {
    let graph = support::journey_graph(records, VENDOR);
    let derived = support::derived(records, VENDOR);
    DerivedJourney::new(&graph, &derived)
        .trace(&key(node))
        .unwrap()
}

fn names(keys: &[NodeKey]) -> Vec<&str> {
    keys.iter().map(NodeKey::as_str).collect()
}

/// C7: the plan's upstream holds its requirement, its two actions, and the kickoff that opens
/// its stage; downstream, its stage, the gated comparison set and the baseline whose relevance
/// that decides, Testing, and everything Reporting holds, with the open, weighted ones marked
/// as its gravity.
#[test]
fn the_plan_traces_across_levels() {
    let traced = trace(&support::vendor_after(1), "n_plan");
    assert_eq!(
        names(&traced.upstream),
        ["n_access", "n_kickoff", "n_plan_draft", "n_plan_review"]
    );
    assert_eq!(
        names(&traced.downstream),
        [
            "n_baseline",
            "n_comparison_set",
            "n_final_report",
            "n_final_review",
            "n_findings",
            "n_findings_reviewer",
            "n_reporting",
            "n_review_opens",
            "n_setup",
            "n_testing",
        ]
    );
    assert_eq!(
        names(&traced.gravity_contributors),
        [
            "n_baseline",
            "n_comparison_set",
            "n_final_report",
            "n_findings",
            "n_findings_reviewer",
            "n_review_opens",
        ],
        "groups weigh nothing"
    );
}

/// C7: once the partner decision is answered no, the partner-led subset is not relevant; the
/// trace still shows it downstream, and it contributes no gravity. A done kickoff is still
/// upstream of what it opened.
#[test]
fn not_relevant_and_terminal_nodes_stay_traceable() {
    let decided = support::vendor_after(3);
    let partner = trace(&decided, "n_partner_runs");
    for subset in [
        "n_criteria",
        "n_partner_led",
        "n_partner_results",
        "n_testing",
    ] {
        assert!(partner.downstream.contains(&key(subset)), "{subset}");
    }
    assert_eq!(partner.gravity_contributors.len(), 0);
    let access = trace(&decided, "n_access");
    assert_eq!(names(&access.upstream), ["n_kickoff"]);
}

#[test]
fn an_unknown_node_is_an_error() {
    let records = support::vendor_after(1);
    let graph = support::journey_graph(&records, VENDOR);
    let derived = support::derived(&records, VENDOR);
    assert_eq!(
        DerivedJourney::new(&graph, &derived).trace(&key("n_nowhere")),
        Err(ProjectionError::UnknownNode(key("n_nowhere")))
    );
}

/// C7: a card standing for hidden nodes traces what they trace: the sets of the card's node and
/// of every node it stands for, joined, less those nodes themselves.
#[test]
fn a_card_standing_for_nodes_traces_what_they_trace() {
    let records = support::vendor_after(1);
    let graph = support::journey_graph(&records, VENDOR);
    let derived = support::derived(&records, VENDOR);
    let journey = DerivedJourney::new(&graph, &derived);
    let standing = [key("n_plan"), key("n_plan_draft"), key("n_plan_review")];
    let joined = journey.trace_standing(&key("n_setup"), &standing).unwrap();
    let own: Vec<NodeKey> = std::iter::once(key("n_setup"))
        .chain(standing.iter().cloned())
        .collect();
    let union = |pick: fn(&Trace) -> &Vec<NodeKey>| -> Vec<NodeKey> {
        let mut all: Vec<NodeKey> = own
            .iter()
            .flat_map(|node| pick(&journey.trace(node).unwrap()).clone())
            .filter(|node| !own.contains(node))
            .collect();
        all.sort();
        all.dedup();
        all
    };
    assert_eq!(joined.upstream, union(|trace| &trace.upstream));
    assert_eq!(joined.downstream, union(|trace| &trace.downstream));
    assert_eq!(
        joined.gravity_contributors,
        union(|trace| &trace.gravity_contributors)
    );
    // The stage alone traces less: the plan's own downstream is not reached from its group.
    assert!(journey.trace(&key("n_setup")).unwrap().downstream.len() < joined.downstream.len());
    assert!(joined.upstream.contains(&key("n_kickoff")));
}
