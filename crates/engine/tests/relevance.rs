//! Pass 1, relevance (PRD Gating; A5; C8): the three decision placements, Kleene evaluation
//! over answers including unanswered decisions, the producer each value names, and force
//! include.
#![cfg(test)]

mod support;

use cairn_engine::derive::{Producer, Relevances};
use cairn_schema::{NodeKey, Relevance};
use support::key;

use Relevance::{NotRelevant, Relevant, Undecided};

fn condition_on(on: &str, decisions: &[&str]) -> Producer {
    Producer::Condition {
        on: key(on),
        decisions: decisions.iter().map(|decision| key(decision)).collect(),
    }
}

fn check(relevances: &Relevances, expected: &[(&str, Relevance, Producer)], label: &str) {
    for (node, value, producer) in expected {
        let found = relevances.get(&key(node)).unwrap();
        assert_eq!(found.value, *value, "{label}: {node}");
        assert_eq!(found.producer, *producer, "{label}: {node}");
    }
}

/// Gating's three placements in the vendor evaluation, and the producer each value names.
#[test]
fn decision_placements_yield_their_relevance() {
    let created = support::derived(&support::vendor_after(1), "j_vendor_eval");
    check(
        created.relevance(),
        &[
            // 1. No open dependencies: relevant from the start.
            ("n_purpose", Relevant, Producer::Unconditioned),
            // 2. Gated by `requires`: structural, still relevant.
            ("n_comparison_set", Relevant, Producer::Unconditioned),
            // 3. Gated by a condition on an open, relevant decision: undecided.
            (
                "n_baseline",
                Undecided,
                condition_on("n_baseline", &["n_comparison_set"]),
            ),
            (
                "n_partner_led",
                Undecided,
                condition_on("n_partner_led", &["n_partner_runs"]),
            ),
            // A child names the ancestor whose condition decides it.
            (
                "n_criteria",
                Undecided,
                condition_on("n_partner_led", &["n_partner_runs"]),
            ),
        ],
        "created",
    );
    let answered = support::derived(&support::vendor_after(2), "j_vendor_eval");
    let partner = condition_on("n_partner_led", &["n_partner_runs"]);
    check(
        answered.relevance(),
        &[
            ("n_partner_led", NotRelevant, partner.clone()),
            ("n_partner_results", NotRelevant, partner),
        ],
        "partner runs: no",
    );
    let compared = support::derived(&support::vendor_after(6), "j_vendor_eval");
    check(
        compared.relevance(),
        &[(
            "n_baseline",
            Relevant,
            condition_on("n_baseline", &["n_comparison_set"]),
        )],
        "comparison set answered",
    );
}

/// A journey with one boolean decision `n_x` and an action per condition shape over it.
fn readers(x_state: &str) -> cairn_engine::Records {
    let journey = support::journey(
        "- op: add_node\n  node: {key: n_x, id: x, kind: decision, title: X, prompt: X?, answer_type: boolean}\n\
- op: add_node\n  node: {key: n_eq, id: eq, kind: action, title: Eq, relevant_when: {equals: {decision: n_x, value: true}}}\n\
- op: add_node\n  node: {key: n_ne, id: ne, kind: action, title: Ne, relevant_when: {not_equals: {decision: n_x, value: true}}}\n\
- op: add_node\n  node: {key: n_not_eq, id: not-eq, kind: action, title: Not eq, relevant_when: {not: {equals: {decision: n_x, value: true}}}}\n\
- op: add_node\n  node: {key: n_answered, id: answered, kind: action, title: Answered, relevant_when: {answered: n_x}}\n",
    );
    if x_state.is_empty() {
        journey
    } else {
        support::accepted(&journey, x_state)
    }
}

/// Kleene evaluation (Gating): an unanswered decision makes value operators false and
/// `not` composes, so `not (x equals yes)` holds when x is skipped; only an open, relevant
/// decision leaves a node undecided.
#[test]
fn conditions_evaluate_three_valued_over_answers() {
    let skip = "- op: transition\n  node: n_x\n  transition: {skip: {reason: Not needed.}}\n";
    let yes = "- op: answer\n  decision: n_x\n  value: {boolean: true}\n";
    let no = "- op: answer\n  decision: n_x\n  value: {boolean: false}\n";
    let cases: &[(&str, &str, [Relevance; 4])] = &[
        ("open", "", [Undecided, Undecided, Undecided, Undecided]),
        (
            "skipped",
            skip,
            [NotRelevant, NotRelevant, Relevant, NotRelevant],
        ),
        ("yes", yes, [Relevant, NotRelevant, NotRelevant, Relevant]),
        ("no", no, [NotRelevant, Relevant, Relevant, Relevant]),
    ];
    for (label, mutation, expected) in cases {
        let derived = support::derived(&readers(mutation), support::JOURNEY);
        let found: Vec<Relevance> = ["n_eq", "n_ne", "n_not_eq", "n_answered"]
            .iter()
            .map(|node| derived.relevance().value(&key(node)))
            .collect();
        assert_eq!(found, expected, "x {label}");
    }
}

/// Gating: a decision that is itself undecided, or open under a skipped container (D1a), is
/// unanswered, so nothing stays undecided on a decision nobody will make.
#[test]
fn decisions_out_of_play_are_unanswered() {
    let base = "- op: add_node\n  node: {key: n_first, id: first, kind: decision, title: First, prompt: First?, answer_type: boolean}\n\
- op: add_node\n  node: {key: n_box, id: box, kind: group, title: Box, relevant_when: {equals: {decision: n_first, value: true}}}\n\
- op: add_node\n  node: {key: n_inner, id: inner, parent: n_box, kind: decision, title: Inner, prompt: Inner?, answer_type: boolean}\n\
- op: add_node\n  node: {key: n_skippable, id: skippable, kind: group, title: Skippable}\n\
- op: add_node\n  node: {key: n_held, id: held, parent: n_skippable, kind: decision, title: Held, prompt: Held?, answer_type: boolean}\n\
- op: add_node\n  node: {key: n_on_inner, id: on-inner, kind: action, title: On inner, relevant_when: {answered: n_inner}}\n\
- op: add_node\n  node: {key: n_on_held, id: on-held, kind: action, title: On held, relevant_when: {answered: n_held}}\n";
    let journey = support::journey(base);
    let before = support::derived(&journey, support::JOURNEY);
    assert_eq!(before.relevance().value(&key("n_inner")), Undecided);
    assert_eq!(
        before.relevance().value(&key("n_on_inner")),
        NotRelevant,
        "an undecided decision is unanswered"
    );
    assert_eq!(before.relevance().value(&key("n_on_held")), Undecided);
    let skip =
        "- op: transition\n  node: n_skippable\n  transition: {skip: {reason: Out of scope.}}\n";
    let skipped = support::derived(&support::accepted(&journey, skip), support::JOURNEY);
    assert_eq!(
        skipped.relevance().value(&key("n_on_held")),
        NotRelevant,
        "an open decision under a skip is unanswered"
    );
    let keep = "- op: apply_override\n  node: n_held\n  override: {keep: {reason: Still asked.}}\n";
    let kept = support::accepted(&support::accepted(&journey, keep), skip);
    let kept = support::derived(&kept, support::JOURNEY);
    assert_eq!(kept.relevance().value(&key("n_on_held")), Undecided);
}

fn force(node: &str) -> String {
    format!(
        "- op: apply_override\n  node: {node}\n  override: {{force_include: {{reason: In scope anyway.}}}}\n"
    )
}

/// Gating, Force include: the node is relevant whatever its condition and ancestors say;
/// its descendants take `relevant` as their ancestor value and still evaluate their own
/// conditions.
#[test]
fn force_include_overrides_conditions_and_ancestors() {
    let not_relevant = support::vendor_after(2);
    let group = support::derived(
        &support::accepted_on(&not_relevant, "j_vendor_eval", &force("n_partner_led")),
        "j_vendor_eval",
    );
    let forced = Producer::Forced {
        on: key("n_partner_led"),
    };
    check(
        group.relevance(),
        &[
            ("n_partner_led", Relevant, forced.clone()),
            ("n_criteria", Relevant, forced),
        ],
        "group forced",
    );
    let child = support::derived(
        &support::accepted_on(&not_relevant, "j_vendor_eval", &force("n_criteria")),
        "j_vendor_eval",
    );
    let relevance = child.relevance();
    assert_eq!(relevance.value(&key("n_criteria")), Relevant);
    assert_eq!(relevance.value(&key("n_partner_results")), NotRelevant);
    assert_eq!(relevance.value(&key("n_partner_led")), NotRelevant);
}

/// E3: a decision's answer is in effect only while it is decided and relevant.
#[test]
fn answers_are_in_effect_while_decided_and_relevant() {
    let records = support::vendor_after(2);
    let derived = support::derived(&records, "j_vendor_eval");
    let graph = support::journey_graph(&records, "j_vendor_eval");
    let in_effect = |decision: &NodeKey| {
        derived
            .relevance()
            .answer_in_effect(graph.document(), decision)
            .is_some()
    };
    assert!(in_effect(&key("n_who_owns")));
    assert!(!in_effect(&key("n_comparison_set")), "open");
    let mut document = graph.document().clone();
    document
        .nodes
        .put({
            let mut owner = document.nodes.get(&key("n_who_owns")).unwrap().clone();
            owner.relevant_when = Some(
                cairn_schema::from_json(
                    r#"{"equals": {"decision": "n_partner_runs", "value": true}}"#,
                )
                .unwrap(),
            );
            owner
        })
        .unwrap();
    let moved = cairn_engine::Graph::new(document).unwrap();
    let inputs = cairn_engine::testing::derive_inputs(records.deployment.clone());
    let out_of_scope = cairn_engine::derive(&moved, &inputs);
    assert!(
        out_of_scope
            .relevance()
            .answer_in_effect(moved.document(), &key("n_who_owns"))
            .is_none(),
        "a decision that leaves scope no longer fills its role"
    );
}
