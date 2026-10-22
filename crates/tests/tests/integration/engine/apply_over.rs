//! A stale patch applied at the current revisions (H5): applied past what intervened when
//! that overlaps nothing it writes, stale otherwise.
#![cfg(test)]

use crate::engine::support;

use cairn_engine::{Records, apply_over};
use cairn_schema::{Patch, Rejection, RevisionConflict, TouchedSet};

const NODES: &str = "\
- op: add_node\n  node: {key: n_a, id: a, kind: action, title: A}\n\
- op: add_node\n  node: {key: n_b, id: b, kind: action, title: B}\n";

const RENAME_A: &str = "- op: set_node_field\n  node: n_a\n  value: {title: Renamed}\n";
const RENAME_B: &str = "- op: set_node_field\n  node: n_b\n  value: {title: Renamed}\n";

/// A journey at revision 2 with two nodes, the patch `mutations` drafted there, and the
/// records and touched set after `meanwhile` landed on top (revision 3).
fn drafted_then_moved(mutations: &str, meanwhile: &str) -> (Records, Patch, TouchedSet) {
    drafted_then_moved_over(NODES, mutations, meanwhile)
}

/// [`drafted_then_moved`] over a journey of `nodes`.
fn drafted_then_moved_over(
    nodes: &str,
    mutations: &str,
    meanwhile: &str,
) -> (Records, Patch, TouchedSet) {
    let drafted = support::journey(nodes);
    let patch = support::patch_to(
        &drafted,
        &format!("{{journey: {}}}", support::JOURNEY),
        mutations,
    );
    let landed = support::patch_to(
        &drafted,
        &format!("{{journey: {}}}", support::JOURNEY),
        meanwhile,
    );
    let landed = support::apply(&drafted, &landed, &support::fixed_inputs()).unwrap();
    let intervening = landed.change_set().touched();
    (landed.records().clone(), patch, intervening)
}

fn conflict_revisions(conflicts: &[RevisionConflict]) -> Vec<(u32, u32)> {
    conflicts
        .iter()
        .map(|conflict| (conflict.expected.get(), conflict.current.get()))
        .collect()
}

#[test]
fn a_patch_disjoint_from_what_intervened_is_applied_at_the_current_revision() {
    let (records, patch, intervening) = drafted_then_moved(RENAME_A, RENAME_B);
    let applied = apply_over(&records, &patch, &support::fixed_inputs(), &intervening).unwrap();
    let receipt = &applied.applied.change_set().receipt;
    assert_eq!(receipt.revision.get(), 4);
    assert_eq!(
        receipt.content_hash,
        patch.content_hash(),
        "the receipt is for the patch as submitted"
    );
    assert_eq!(applied.at.base_revision.get(), 3);
    let over = applied.over.expect("applied past a moved revision");
    assert_eq!(conflict_revisions(&over.conflicts), [(2, 3)]);
    assert_eq!(over.intervening, intervening);
    let titles: Vec<_> = support::graph(applied.applied.records())
        .nodes
        .as_map()
        .values()
        .map(|node| node.title.to_string())
        .collect();
    assert_eq!(titles, ["Renamed", "Renamed"], "the other change is kept");
}

#[test]
fn a_patch_writing_what_intervened_wrote_is_stale_with_both_sets() {
    let (records, patch, intervening) = drafted_then_moved(RENAME_A, RENAME_A);
    let Err(Rejection::Stale {
        conflicts,
        intervening: seen,
        touched,
    }) = apply_over(&records, &patch, &support::fixed_inputs(), &intervening)
    else {
        panic!("what intervened overlaps what the patch writes");
    };
    assert_eq!(conflict_revisions(&conflicts), [(2, 3)]);
    assert_eq!(seen, intervening);
    assert!(seen.overlaps(&touched));
}

/// Never applied past a moved revision: each case is answered stale with its whole domain.
#[test]
fn a_patch_that_is_not_safe_to_move_is_stale_with_its_whole_domain() {
    let (records, patch, _) = drafted_then_moved(RENAME_A, RENAME_B);
    let none = TouchedSet::default();
    let whole = TouchedSet::whole(patch.target.domain());
    let mut ahead = patch.clone();
    ahead.base_revision = 9.try_into().unwrap();
    let mut duplicate = patch.clone();
    duplicate.mutations = support::patch_to(
        &records,
        &format!("{{journey: {}}}", support::JOURNEY),
        "- op: add_node\n  node: {key: n_a, id: a2, kind: action, title: Again}\n",
    )
    .mutations;
    for (case, patch) in [
        ("a base past the current revision", ahead),
        ("a patch the current records refuse", duplicate),
    ] {
        let answer = apply_over(&records, &patch, &support::fixed_inputs(), &none);
        let Err(Rejection::Stale { touched, .. }) = answer else {
            panic!("{case}: stale, not {answer:#?}");
        };
        assert_eq!(touched, whole, "{case}");
    }
}

#[test]
fn a_patch_applying_a_proposal_is_never_moved() {
    let drafted = support::after("bake-off", 2);
    let apply_it = "- op: apply_proposal\n  proposal: pr_bakeoff\n  reviewed_revision: 1\n";
    let patch = support::patch_to(&drafted, "{journey: j_bakeoff}", apply_it);
    let moved = support::accepted_on(
        &drafted,
        "j_bakeoff",
        "- op: edit_journey\n  name: Two-week bake-off between options\n",
    );
    let answer = apply_over(
        &moved,
        &patch,
        &support::fixed_inputs(),
        &TouchedSet::default(),
    );
    let Err(Rejection::Stale { touched, .. }) = answer else {
        panic!("stale, not {answer:#?}");
    };
    assert_eq!(touched, TouchedSet::whole(patch.target.domain()));
}

#[test]
fn a_current_patch_is_applied_as_it_is() {
    let drafted = support::journey(NODES);
    let patch = support::patch_to(
        &drafted,
        &format!("{{journey: {}}}", support::JOURNEY),
        RENAME_A,
    );
    let applied = apply_over(
        &drafted,
        &patch,
        &support::fixed_inputs(),
        &TouchedSet::default(),
    )
    .unwrap();
    assert_eq!(applied.over, None);
    assert_eq!(applied.at, patch);
}

const CHECKLIST: &str = "- op: add_node\n  node: {key: n_a, id: a, kind: action, title: A, checklist: [{id: x, title: X}, {id: y, title: Y}]}\n";
const TICK_X: &str = "- op: set_ticked\n  node: n_a\n  item: x\n  ticked: true\n";
const TICK_Y: &str = "- op: set_ticked\n  node: n_a\n  item: y\n  ticked: true\n";
const REPLACE_LIST: &str =
    "- op: set_node_field\n  node: n_a\n  value: {checklist: [{id: x, title: Another X}]}\n";

/// G5: a tick drafted before an edit of its node's checklist is stale. The edit can drop the
/// item and a later one reuse its id, so the tick could land on a different item.
#[test]
fn a_tick_is_stale_past_an_edit_of_its_nodes_checklist() {
    let (records, patch, intervening) = drafted_then_moved_over(CHECKLIST, TICK_X, REPLACE_LIST);
    let answer = apply_over(&records, &patch, &support::fixed_inputs(), &intervening);
    assert!(
        matches!(answer, Err(Rejection::Stale { .. })),
        "{answer:#?}"
    );
}

/// G5: ticks of two items of one node are different records, and are applied past each other.
#[test]
fn a_tick_is_applied_past_a_tick_of_another_item() {
    let (records, patch, intervening) = drafted_then_moved_over(CHECKLIST, TICK_X, TICK_Y);
    let applied = apply_over(&records, &patch, &support::fixed_inputs(), &intervening).unwrap();
    let ticked: Vec<_> = support::graph(applied.applied.records())
        .state
        .ticks
        .values()
        .flatten()
        .map(ToString::to_string)
        .collect();
    assert_eq!(ticked, ["x", "y"]);
}
