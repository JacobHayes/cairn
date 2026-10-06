//! Effective skip and kept work (D1a): a container's skip reaches its non-terminal, in-scope
//! descendants through containment only, `keep` exempts a subtree, reopening restores what
//! was inherited but not what was skipped explicitly, and each skipped container lists the
//! kept work beneath it.
#![cfg(test)]

mod support;

use std::collections::BTreeSet;

use cairn_engine::Derived;
use cairn_schema::NodeKey;
use support::key;

const VENDOR: &str = "j_vendor_eval";

fn skip(node: &str) -> String {
    format!("- op: transition\n  node: {node}\n  transition: {{skip: {{reason: Not needed.}}}}\n")
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
fn a_skip_reaches_descendants_through_containment_only() {
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
