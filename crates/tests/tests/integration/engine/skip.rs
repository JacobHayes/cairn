//! Effective skip and kept work (D1a): a container's skip, or a group marked done, reaches its non-terminal, in-scope
//! descendants through containment only, `keep` exempts a subtree, reopening restores what
//! was inherited but not what was skipped explicitly, and each skipped container lists the
//! kept work beneath it.
#![cfg(test)]

use crate::engine::support;

use std::collections::BTreeSet;

use cairn_engine::Derived;
use cairn_schema::{DisplayState, NodeKey, State};
use support::key;

const VENDOR: &str = "j_vendor_eval";

fn skip(node: &str) -> String {
    format!("- op: transition\n  node: {node}\n  transition: {{skip: {{reason: Not needed.}}}}\n")
}

fn mark_done(node: &str) -> String {
    format!(
        "- op: transition\n  node: {node}\n  transition: {{mark_done: {{reason: Covered elsewhere.}}}}\n"
    )
}

fn complete(node: &str) -> String {
    format!("- op: transition\n  node: {node}\n  transition: complete\n")
}

fn keep(node: &str) -> String {
    format!(
        "- op: apply_override\n  node: {node}\n  override: {{keep: {{reason: Still needed.}}}}\n"
    )
}

fn reopen(node: &str) -> String {
    format!("- op: transition\n  node: {node}\n  transition: reopen\n")
}

/// The vendor evaluation after kickoff, with access started, then `mutations`.
fn vendor_records(mutations: &[String]) -> cairn_engine::Records {
    let mut records = support::vendor_after(4);
    for mutation in mutations {
        records = support::accepted_on(&records, VENDOR, mutation);
    }
    records
}

fn vendor(mutations: &[String]) -> Derived {
    support::derived(&vendor_records(mutations), VENDOR)
}

fn skipped_by(derived: &Derived, node: &str) -> Option<String> {
    derived
        .skips()
        .skipped_by(&key(node))
        .map(|source| source.as_str().to_owned())
}

fn keys(names: &[&str]) -> BTreeSet<NodeKey> {
    names.iter().map(|name| key(name)).collect()
}

#[test]
fn a_skip_donees_descendants_through_containment_only() {
    let derived = vendor(&[skip("n_plan")]);
    for child in ["n_plan_draft", "n_plan_review"] {
        assert_eq!(
            skipped_by(&derived, child).as_deref(),
            Some("n_plan"),
            "{child}"
        );
    }
    assert_eq!(
        skipped_by(&derived, "n_comparison_set"),
        None,
        "a dependent of the skipped plan is not skipped"
    );
    assert_eq!(
        skipped_by(&derived, "n_plan"),
        None,
        "the plan's own skip is stored"
    );
}

#[test]
fn keep_exempts_a_subtree_and_becomes_kept_work() {
    let derived = vendor(&[keep("n_plan_review"), skip("n_setup")]);
    assert_eq!(skipped_by(&derived, "n_plan_review"), None);
    assert_eq!(skipped_by(&derived, "n_plan").as_deref(), Some("n_setup"));
    assert_eq!(
        skipped_by(&derived, "n_access").as_deref(),
        Some("n_setup"),
        "started work is non-terminal, so it is effectively skipped too"
    );
    let skips = derived.skips();
    assert_eq!(skips.kept_work(&key("n_setup")), &keys(&["n_plan_review"]));
    assert_eq!(
        skips.kept_work(&key("n_plan")),
        &keys(&["n_plan_review"]),
        "an effectively skipped intermediate container holds the kept work too"
    );
    assert!(skips.kept_work(&key("n_plan_draft")).is_empty());
}

/// D1a, D4: an effectively skipped intermediate container satisfies its dependents only
/// once the kept work beneath it does, and the effectively skipped work leaves the
/// frontier.
#[test]
fn a_cascaded_skip_holds_dependents_on_the_kept_work() {
    let pending = vendor(&[keep("n_plan_review"), skip("n_setup")]);
    let blocking = pending.blocking();
    assert!(!blocking.satisfies(&key("n_plan")), "kept work pending");
    assert!(blocking.blocked(&key("n_comparison_set")));
    assert!(blocking.frontier().contains(&key("n_plan_review")));
    for skipped in ["n_access", "n_plan_draft"] {
        assert!(!blocking.frontier().contains(&key(skipped)), "{skipped}");
    }
    let done = vendor(&[
        keep("n_plan_review"),
        skip("n_setup"),
        "- op: transition\n  node: n_plan_review\n  transition: complete\n".to_owned(),
    ]);
    assert!(done.blocking().satisfies(&key("n_plan")));
    assert!(done.blocking().actionable(&key("n_comparison_set")));
}

#[test]
fn only_in_scope_work_is_skipped_or_kept() {
    // After step 4 the partner-led branch is not relevant (partner runs: no).
    let both = vendor(&[
        keep("n_criteria"),
        keep("n_comparison_set"),
        keep("n_baseline"),
        skip("n_testing"),
    ]);
    assert_eq!(skipped_by(&both, "n_partner_results"), None);
    assert_eq!(
        both.skips().kept_work(&key("n_testing")),
        &keys(&["n_baseline", "n_comparison_set"]),
        "the kept criteria are not relevant, so they are not pending work"
    );
    // Without its decision kept, the baseline reads an effectively skipped decision as
    // unanswered (Gating), so it is not relevant and not pending either.
    let baseline = vendor(&[keep("n_baseline"), skip("n_testing")]);
    assert_eq!(
        skipped_by(&baseline, "n_comparison_set").as_deref(),
        Some("n_testing")
    );
    assert!(baseline.skips().kept_work(&key("n_testing")).is_empty());
}

#[test]
fn reopening_restores_inherited_skips_but_not_explicit_ones() {
    let records = vendor_records(&[skip("n_plan_review"), skip("n_plan"), reopen("n_plan")]);
    let derived = support::derived(&records, VENDOR);
    assert_eq!(skipped_by(&derived, "n_plan_draft"), None, "restored");
    let stored = &support::vendor_graph(&records).state.nodes[&key("n_plan_review")];
    assert_eq!(
        stored.state,
        cairn_schema::State::Skipped,
        "the explicit skip stays"
    );
}

/// D1a: every explicitly kept, in-scope node beneath a skipped container is its kept work,
/// nested keeps included: a kept node's own state need not cover a kept child (here it is
/// not relevant while the child is force-included).
#[test]
fn nested_keeps_are_each_kept_work() {
    let nested = vendor(&[keep("n_plan"), keep("n_plan_review"), skip("n_setup")]);
    assert_eq!(
        nested.skips().kept_work(&key("n_setup")),
        &keys(&["n_plan", "n_plan_review"])
    );
    let journey = support::journey(
        "- op: add_node\n  node: {key: n_flag, id: flag, kind: decision, title: Flag, prompt: Flag?, answer_type: boolean}\n\
- op: add_node\n  node: {key: n_outer, id: outer, kind: group, title: Outer}\n\
- op: add_node\n  node: {key: n_mid, id: mid, parent: n_outer, kind: group, title: Mid, relevant_when: {equals: {decision: n_flag, value: true}}}\n\
- op: add_node\n  node: {key: n_inner, id: inner, parent: n_mid, kind: action, title: Inner}\n\
- op: answer\n  decision: n_flag\n  value: {boolean: false}\n",
    );
    let journey = support::accepted(
        &journey,
        &format!(
            "{}{}- op: apply_override\n  node: n_inner\n  override: {{force_include: {{reason: Needed.}}}}\n{}",
            keep("n_mid"),
            keep("n_inner"),
            skip("n_outer")
        ),
    );
    let derived = support::derived(&journey, support::JOURNEY);
    assert_eq!(
        derived.skips().kept_work(&key("n_outer")),
        &keys(&["n_inner"])
    );
}

fn stored_state(records: &cairn_engine::Records, node: &str) -> State {
    support::vendor_graph(records).state.nodes[&key(node)].state
}

fn shown(records: &cairn_engine::Records, derived: &Derived, node: &str) -> DisplayState {
    derived.display_state(&support::journey_graph(records, VENDOR), &key(node))
}

/// D1a: marking a group done skips what is unfinished inside it and leaves finished work and
/// every stored state alone.
#[test]
fn marking_a_group_done_skips_what_is_unfinished_inside() {
    let before = vendor_records(&[complete("n_access")]);
    let records = support::accepted_on(&before, VENDOR, &mark_done("n_setup"));
    let derived = support::derived(&records, VENDOR);
    for unfinished in ["n_plan", "n_plan_draft", "n_plan_review", "n_workload"] {
        assert_eq!(
            skipped_by(&derived, unfinished).as_deref(),
            Some("n_setup"),
            "{unfinished}"
        );
        assert_eq!(shown(&records, &derived, unfinished), DisplayState::Skipped);
    }
    assert_eq!(skipped_by(&derived, "n_access"), None, "finished");
    assert_eq!(shown(&records, &derived, "n_access"), DisplayState::Done);
    for descendant in ["n_access", "n_plan", "n_plan_draft", "n_plan_review"] {
        assert_eq!(
            stored_state(&records, descendant),
            stored_state(&before, descendant),
            "{descendant}"
        );
    }
    assert_eq!(shown(&records, &derived, "n_setup"), DisplayState::Done);
    assert!(
        !derived.blocking().blocked(&key("n_comparison_set")),
        "the plan it requires reads skipped, so it no longer holds the comparison set"
    );
}

/// D1a, D4: a group marked done satisfies its dependents only once the kept work inside it
/// does. The terminal shortcut would satisfy it at once.
#[test]
fn a_group_marked_done_holds_its_dependents_on_kept_work() {
    let add = "- op: add_node\n  node: {key: n_after, id: after, kind: action, title: After, requires: [n_setup]}\n";
    let records = vendor_records(&[add.to_owned(), keep("n_plan_review"), mark_done("n_setup")]);
    let derived = support::derived(&records, VENDOR);
    let blocking = derived.blocking();
    assert!(!blocking.satisfies(&key("n_setup")));
    assert!(blocking.blocked(&key("n_after")));
    assert!(blocking.frontier().contains(&key("n_plan_review")));
    let graph = support::journey_graph(&records, VENDOR);
    assert_eq!(
        derived
            .node_derived(&graph, &key("n_setup"))
            .kept_work_pending,
        vec![key("n_plan_review")]
    );
    let finished = support::accepted_on(&records, VENDOR, &complete("n_plan_review"));
    let derived = support::derived(&finished, VENDOR);
    assert!(derived.blocking().actionable(&key("n_after")));
    assert_eq!(
        derived
            .node_derived(&support::journey_graph(&finished, VENDOR), &key("n_setup"))
            .kept_work_pending,
        [] as [cairn_schema::NodeKey; 0]
    );
}

#[test]
fn nested_keeps_under_a_group_marked_done_are_each_kept_work() {
    let derived = vendor(&[keep("n_plan"), keep("n_plan_review"), mark_done("n_setup")]);
    assert_eq!(
        derived.skips().kept_work(&key("n_setup")),
        &keys(&["n_plan", "n_plan_review"])
    );
}

#[test]
fn work_added_after_a_mark_done_reads_skipped() {
    let add = "- op: add_node\n  node: {key: n_late, id: late, parent: n_setup, kind: action, title: Late}\n";
    let derived = vendor(&[mark_done("n_setup"), add.to_owned()]);
    assert_eq!(skipped_by(&derived, "n_late").as_deref(), Some("n_setup"));
}

/// D1a: reopening removes this group's mark only: skips and keeps inside, and a mark deeper
/// inside, stay as they were.
#[test]
fn reopening_a_group_marked_done_removes_only_its_mark() {
    let records = vendor_records(&[
        keep("n_comparison_set"),
        skip("n_baseline"),
        mark_done("n_partner_led"),
        mark_done("n_testing"),
        reopen("n_testing"),
    ]);
    let derived = support::derived(&records, VENDOR);
    let stored = &support::vendor_graph(&records).state.nodes[&key("n_testing")];
    assert_eq!(stored.state, State::Derived);
    assert!(stored.reason.is_none());
    for node in ["n_comparison_set", "n_baseline", "n_partner_led"] {
        assert_ne!(
            skipped_by(&derived, node).as_deref(),
            Some("n_testing"),
            "{node}"
        );
    }
    assert_eq!(stored_state(&records, "n_baseline"), State::Skipped);
    assert_eq!(stored_state(&records, "n_partner_led"), State::Done);
    assert!(
        support::vendor_graph(&records).state.overrides[&key("n_comparison_set")]
            .keep
            .is_some()
    );
}

/// D4: marking a group done is never guarded: with its opening unmet it is accepted and not
/// stale, and kept work inside still waits for the opening it inherits.
#[test]
fn marking_done_is_not_guarded() {
    let records = support::accepted_on(
        &support::vendor_after(1),
        VENDOR,
        &format!("{}{}", keep("n_plan_review"), mark_done("n_setup")),
    );
    let derived = support::derived(&records, VENDOR);
    let graph = support::journey_graph(&records, VENDOR);
    assert!(
        derived
            .node_derived(&graph, &key("n_setup"))
            .stale
            .is_empty()
    );
    assert!(derived.blocking().blocked(&key("n_plan_review")));
}

/// D1: completing work that has children leaves the children open: they are work of their
/// own, so nothing cascades from a done deliverable.
#[test]
fn completing_work_with_children_skips_nothing() {
    let derived = vendor(&[complete("n_access"), complete("n_plan")]);
    assert_eq!(skipped_by(&derived, "n_plan_draft"), None);
    assert!(derived.blocking().frontier().contains(&key("n_plan_draft")));
}
