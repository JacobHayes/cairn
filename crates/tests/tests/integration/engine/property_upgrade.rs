//! Property tests for upgrade (B7, J3), over journeys created from the vendor
//! evaluation's version 1 and changed by generated operations (local nodes and edges,
//! answers, transitions, pins, snoozes, removals that leave tombstones): upgrading to an
//! identical version proposes nothing beyond listing kept edits and changes nothing but the
//! lineage; upgrading to
//! version 2 with every conflict kept and every orphan kept applies exactly when the draft
//! lists no violation, and applying it then replaying its events equals the state; and a
//! journey saved as a route, published, and created again has its nodes back by key (B8), but
//! the breakdown children excluded by default.

#[cfg(test)]
mod property {
    use cairn_engine::testing::{arb_ops, journey_id};
    use cairn_engine::{Records, apply, replay, save_as_route, upgrade};
    use cairn_schema::{ConflictResolution, ProposalDraft, ReviewItem, VersionNumber};
    use patina_dst_proptest::prelude::*;

    use crate::engine::support;

    const JOURNEY: &str = "{journey: j_generated}";

    /// The vendor route at version 1 and a journey created from it, then the operations.
    fn journey(ops: &[cairn_engine::testing::Op]) -> Records {
        let seeded = support::seeded("vendor-evaluation");
        let create = "- op: create_journey\n  name: Generated\n  from: {route: vendor-evaluation, version: 1}\n";
        let patch = support::patch_to(&seeded, JOURNEY, create);
        let mut records = apply(&seeded, &patch, &support::fixed_inputs())
            .unwrap()
            .records()
            .clone();
        for op in ops {
            if let Ok(applied) = apply(&records, &op.resolve(&records), &support::fixed_inputs()) {
                records = applied.records().clone();
            }
        }
        records
    }

    fn two() -> VersionNumber {
        VersionNumber::FIRST.next()
    }

    fn violations(draft: &ProposalDraft) -> usize {
        draft
            .items
            .as_slice()
            .iter()
            .filter(|item| matches!(item, ReviewItem::Violation { .. }))
            .count()
    }

    fn kept(draft: &ProposalDraft) -> ProposalDraft {
        let items = draft.items.as_slice().iter().map(|item| match item {
            ReviewItem::Conflict { conflict, .. } => ReviewItem::Conflict {
                conflict: conflict.clone(),
                resolution: Some(ConflictResolution::KeepJourney),
            },
            other => other.clone(),
        });
        ProposalDraft {
            items: cairn_schema::BoundedVec::new(items.collect()).unwrap(),
            ..draft.clone()
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// B7: an upgrade to a version identical to the one followed proposes nothing (it only
        /// lists the journey's local edits as kept) and changes nothing but the lineage.
        #[test]
        fn an_identical_version_proposes_nothing(ops in prop::collection::vec(arb_ops(), 0..40)) {
            let records = journey(&ops);
            let same = support::route_graph("vendor-evaluation").into_document();
            let records = support::publish_vendor(&records, same);
            let draft = upgrade(&records, &journey_id(), two(), &support::fixed_inputs()).unwrap();
            let only_kept = draft
                .items
                .as_slice()
                .iter()
                .all(|item| matches!(item, ReviewItem::KeptLocalEdit { .. }));
            prop_assert!(only_kept, "{:#?}", draft.items);
            let proposed = support::propose(&records, "pr_upgrade", JOURNEY, &draft);
            let applied = support::apply_proposal(&proposed, "pr_upgrade").unwrap();
            let before = &records.journeys[&journey_id()].graph;
            let after = &applied.records().journeys[&journey_id()].graph;
            prop_assert_eq!(before, after);
        }

        /// B7, J3: keeping every conflict and orphan applies exactly when the draft lists no
        /// violation, and replaying the applied events equals the state.
        #[test]
        fn a_kept_upgrade_applies_unless_it_lists_a_violation(ops in prop::collection::vec(arb_ops(), 0..40)) {
            let records = support::publish_vendor(&journey(&ops), support::vendor_v2());
            let draft = upgrade(&records, &journey_id(), two(), &support::fixed_inputs()).unwrap();
            let proposed = support::propose(&records, "pr_upgrade", JOURNEY, &kept(&draft));
            match support::apply_proposal(&proposed, "pr_upgrade") {
                Ok(applied) => {
                    prop_assert_eq!(violations(&draft), 0);
                    prop_assert_eq!(&replay(&proposed, applied.events()), applied.records());
                    let header = &applied.records().journeys[&journey_id()].header;
                    prop_assert_eq!(header.lineage.as_ref().unwrap().version, two());
                }
                Err(rejection) => prop_assert!(violations(&draft) > 0, "{rejection:#?}"),
            }
        }
        /// B8, B1: save as route, publish, and create again: every node not excluded comes
        /// back by key, its edges into excluded nodes dropped.
        #[test]
        fn a_saved_journey_is_created_again_by_key(ops in prop::collection::vec(arb_ops(), 0..40)) {
            let records = journey(&ops);
            let draft = save_as_route(&records, &journey_id(), &"saved".parse().unwrap(), &"Saved".parse().unwrap(), &support::fixed_inputs()).unwrap();
            let proposed = support::propose(&records, "pr_save", "{route: saved}", &draft);
            let applied = match support::apply_proposal(&proposed, "pr_save") {
                Ok(applied) => applied.records().clone(),
                Err(rejection) => {
                    prop_assert!(violations(&draft) > 0, "{rejection:#?}");
                    return Ok(());
                }
            };
            let published = support::accepted_to(&applied, "{route: saved}", "- op: publish_draft\n");
            let again = support::accepted_to(&published, "{journey: j_again}", "- op: create_journey\n  name: Again\n  from: {route: saved, version: 1}\n");
            let original = &records.journeys[&journey_id()].graph;
            let tree = cairn_engine::Tree::build(original);
            let mut excluded = std::collections::BTreeSet::new();
            for item in draft.items.as_slice() {
                if let ReviewItem::Exclusion { node, excluded: true } = item {
                    excluded.insert(node.clone());
                    excluded.extend(tree.descendants(node));
                }
            }
            let expected: Vec<_> = original.nodes.values().filter(|node| !excluded.contains(&node.key)).map(|node| {
                let mut node = node.clone();
                let kept: Vec<_> = node.requires.iter().filter(|key| !excluded.contains(*key)).cloned().collect();
                node.requires = cairn_schema::BoundedSet::new(kept).unwrap();
                node
            }).collect();
            let created: Vec<_> = again.journeys[&"j_again".parse().unwrap()].graph.nodes.values().cloned().collect();
            prop_assert_eq!(created, expected);
        }
    }
}
