//! Proposal documents (C14, I6; B7, B8): review items resolve into ordinary mutations, and only
//! an item that still needs a choice blocks apply.
#![cfg(test)]

use crate::engine::support;

use cairn_engine::{resolve, resolve_partial};
use cairn_schema::{
    Mutation, NodeFieldValue, ParticipationSource, UnresolvedReason, ViolationCode,
};

const JOURNEY: &str = "{journey: j_vendor_eval}";

/// A field conflict on the access deliverable's title, resolved as given.
fn title_conflict(resolution: &str) -> String {
    format!(
        "title: Take the route's title\ndestination_revision: 1\nitems:\n\
         - item: conflict\n  conflict: {{about: field, node: n_access, journey: {{title: Access}}, route: {{title: Environment and data access}}}}\n{resolution}"
    )
}

/// C14, I6: an unresolved conflict blocks apply, about its node; once the reviewer takes
/// the route's value, apply writes it and leaves no local-edit marker.
#[test]
fn an_unresolved_conflict_blocks_apply_until_it_is_chosen() {
    let records = support::vendor_after(1);
    let open = support::propose(
        &records,
        "pr_title",
        JOURNEY,
        &support::draft(&title_conflict("")),
    );
    let blocked = support::apply_proposal(&open, "pr_title");
    assert_eq!(
        support::codes(blocked),
        [ViolationCode::UnresolvedReviewItem]
    );

    let chosen = support::draft(&title_conflict("  resolution: take_route\n"));
    let records = support::propose(&records, "pr_title", JOURNEY, &chosen);
    let applied = support::apply_proposal(&records, "pr_title").unwrap();
    let graph = support::vendor_graph(applied.records());
    let access = graph.nodes.get(&support::key("n_access")).unwrap();
    assert_eq!(access.title.as_str(), "Environment and data access");
    assert!(
        !graph
            .state
            .local_edits
            .contains_key(&support::key("n_access"))
    );
}

/// B4, B7: keeping the journey's value marks it as a local edit, so a later upgrade keeps it
/// too; a resolution the conflict does not offer blocks like a missing one.
#[test]
fn keeping_marks_the_edit_and_an_unoffered_choice_blocks() {
    let kept = resolve(&support::draft(&title_conflict(
        "  resolution: keep_journey\n",
    )));
    let title = cairn_schema::LocalEdit::Field(cairn_schema::NodeField::Title);
    assert_eq!(
        kept,
        Ok(vec![Mutation::MarkLocalEdit {
            node: support::key("n_access"),
            edit: title,
            marked: true
        }])
    );
    let cleared = resolve(&support::draft(&title_conflict(
        "  resolution: clear_state\n",
    )));
    let reasons: Vec<_> = cleared
        .unwrap_err()
        .iter()
        .map(|item| item.reason)
        .collect();
    assert_eq!(reasons, [UnresolvedReason::NotOffered]);
}

/// The first mutation of each resolution of an invalidated single-choice answer.
#[test]
fn an_invalidated_answer_maps_keeps_or_reopens() {
    let conflict = "- item: conflict\n  conflict: {about: answer, decision: n_purpose, answer: {single_choice: research-only}, choices: [purchase, evaluate]}\n";
    let cases = [
        ("keep_journey", "mark_local_edit"),
        (
            "{map_choices: {map: {research-only: evaluate}}}",
            "set_node_field",
        ),
        // A single choice cleared of its only choice has no answer left.
        ("clear_state", "transition"),
        ("reopen", "transition"),
    ];
    for (resolution, first) in cases {
        let yaml = format!(
            "title: Choices\ndestination_revision: 1\nitems:\n{conflict}  resolution: {resolution}\n"
        );
        let mutations = resolve(&support::draft(&yaml)).unwrap();
        let op = serde_json::to_value(&mutations[0]).unwrap()["op"].clone();
        assert_eq!(op, first, "{resolution}");
    }
    let unmapped = "title: Choices\ndestination_revision: 1\nitems:\n".to_owned()
        + conflict
        + "  resolution: {map_choices: {map: {purchase: evaluate}}}\n";
    assert!(
        resolve(&support::draft(&unmapped)).is_err(),
        "maps a kept choice"
    );
}

/// A save-as-route draft: a group with a child and a third node requiring the child, the
/// group's owner an explicit entity.
fn saved(items: &str) -> String {
    format!(
        "title: Save\ndestination_revision: 0\nmutations:\n\
         - op: create_route\n  name: Saved\n\
         - op: open_draft\n  source: {{save_as_route: {{journey: j_vendor_eval}}}}\n\
         - op: add_node\n  node: {{key: n_group, id: group, kind: group, title: Group, participations: {{k_owner: [e_lead, e_other]}}}}\n\
         - op: add_node\n  node: {{key: n_child, id: child, parent: n_group, kind: action, title: Child}}\n\
         - op: add_node\n  node: {{key: n_after, id: after, kind: action, title: After, requires: [n_child]}}\n\
         items:\n{items}"
    )
}

fn added(mutations: &[Mutation]) -> Vec<&cairn_schema::Node<cairn_schema::KeyRefs>> {
    mutations
        .iter()
        .filter_map(|mutation| match mutation {
            Mutation::AddNode { node } => Some(node),
            _ => None,
        })
        .collect()
}

/// B8: an excluded node goes with its subtree and the edges into it; explicit entities
/// mapped to one role become that role, and a new role is added.
#[test]
fn exclusions_and_mappings_rewrite_the_saved_structure() {
    let items = "\
- item: participation\n  entity: e_lead\n  uses: [{node: n_group, kind: k_owner}]\n  mapping: {new_role: {key: r_lead, id: lead}}\n\
- item: participation\n  entity: e_other\n  uses: [{node: n_group, kind: k_owner}]\n  mapping: drop\n\
- item: exclusion\n  node: n_group\n  excluded: false\n\
- item: exclusion\n  node: n_child\n  excluded: true\n";
    let mutations = resolve(&support::draft(&saved(items))).unwrap();
    let nodes = added(&mutations);
    let keys: Vec<&str> = nodes.iter().map(|node| node.key.as_str()).collect();
    assert_eq!(keys, ["n_group", "n_after"]);
    assert!(
        nodes[1].requires.is_empty(),
        "the edge into the excluded child goes"
    );
    let owner = nodes[0].participations.as_map().values().next().unwrap();
    assert_eq!(*owner, ParticipationSource::Role("r_lead".parse().unwrap()));
    assert!(mutations.iter().any(
        |mutation| matches!(mutation, Mutation::AddRole { role } if role.key.as_str() == "r_lead")
    ));
}

/// B8: entities on one participation mapped to different roles block both items; one mapped
/// to the default owner of a graph with none blocks.
#[test]
fn mixed_mappings_and_a_missing_default_owner_block() {
    let mixed = "\
- item: participation\n  entity: e_lead\n  uses: [{node: n_group, kind: k_owner}]\n  mapping: {role: r_one}\n\
- item: participation\n  entity: e_other\n  uses: [{node: n_group, kind: k_owner}]\n  mapping: {role: r_two}\n";
    let blocked = resolve_partial(&support::draft(&saved(mixed))).unwrap_or_default_unresolved();
    assert_eq!(
        blocked,
        [
            (0, UnresolvedReason::MixedMapping),
            (1, UnresolvedReason::MixedMapping)
        ]
    );
    let owner = "\
- item: participation\n  entity: e_lead\n  uses: [{node: n_group, kind: k_owner}]\n  mapping: default_owner\n\
- item: participation\n  entity: e_other\n  uses: [{node: n_group, kind: k_owner}]\n  mapping: drop\n";
    let blocked = resolve_partial(&support::draft(&saved(owner))).unwrap_or_default_unresolved();
    assert_eq!(blocked, [(0, UnresolvedReason::NoDefaultOwner)]);
}

trait Blocked {
    fn unwrap_or_default_unresolved(self) -> Vec<(u32, UnresolvedReason)>;
}

impl Blocked for cairn_engine::proposal::Resolved {
    fn unwrap_or_default_unresolved(self) -> Vec<(u32, UnresolvedReason)> {
        self.unresolved
            .into_iter()
            .map(|item| (item.item, item.reason))
            .collect()
    }
}

/// B7: removing an orphan inside another removed orphan is the outer removal's.
#[test]
fn nested_orphan_removals_remove_once() {
    let yaml = "title: Orphans\ndestination_revision: 1\nitems:\n\
- item: orphan\n  node: n_outer\n  keep: false\n  removal: {node: n_outer, descendants: [n_inner]}\n\
- item: orphan\n  node: n_inner\n  keep: false\n  removal: {node: n_inner}\n\
- item: orphan\n  node: n_kept\n  keep: true\n  removal: {node: n_kept}\n";
    let mutations = resolve(&support::draft(yaml)).unwrap();
    let removed: Vec<&str> = mutations
        .iter()
        .filter_map(|mutation| match mutation {
            Mutation::RemoveNode { removal } => Some(removal.node.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(removed, ["n_outer"]);
}

/// B7: a removed role the journey still uses is remapped reference by reference, then removed.
#[test]
fn a_removed_role_is_remapped_then_removed() {
    let yaml = "title: Roles\ndestination_revision: 1\nitems:\n\
- item: conflict\n  conflict: {about: role, role: r_gone, journey: {key: r_gone, id: gone}, references: [{fills_role: {node: n_who}}, default_owner]}\n  resolution: {remap_role: {role: r_kept}}\n";
    let mutations = resolve(&support::draft(yaml)).unwrap();
    assert!(mutations.contains(&Mutation::SetNodeField {
        node: support::key("n_who"),
        value: NodeFieldValue::FillsRole(Some("r_kept".parse().unwrap())),
    }));
    assert_eq!(
        mutations.last(),
        Some(&Mutation::RemoveRole {
            role: "r_gone".parse().unwrap()
        })
    );
}

/// B7: a removed choice mapped onto one the multi-choice answer already names leaves that
/// one choice answered, not a reopened decision.
#[test]
fn a_choice_mapped_onto_a_chosen_one_merges_into_it() {
    let yaml = "title: Choices\ndestination_revision: 1\nitems:\n\
- item: conflict\n  conflict: {about: answer, decision: n_purpose, answer: {multi_choice: [purchase, evaluate]}, choices: [evaluate, other]}\n  resolution: {map_choices: {map: {purchase: evaluate}}}\n";
    let mutations = resolve(&support::draft(yaml)).unwrap();
    let answered: cairn_schema::AnswerValue =
        cairn_schema::from_yaml("multi_choice: [evaluate]").unwrap();
    assert!(mutations.contains(&Mutation::Answer {
        decision: support::key("n_purpose"),
        value: answered,
        rationale: None,
    }));
    assert!(
        !mutations
            .iter()
            .any(|mutation| matches!(mutation, Mutation::Transition { .. }))
    );
}

/// B7: a removed role's message drafts are rewritten with it: remapped to the other role's
/// placeholder, or, removed, the role's name written out.
#[test]
fn a_removed_role_rewrites_the_drafts_naming_it() {
    let conflict = "- item: conflict\n  conflict: {about: role, role: r_gone, journey: {key: r_gone, id: gone, title: Approver}, references: [{draft: {node: n_local, resource: {key: a_msg, message_draft: 'Hi {{roles.r_gone.name}}'}}}]}\n";
    let drafted = |resolution: &str| {
        let yaml = format!(
            "title: Roles\ndestination_revision: 1\nitems:\n{conflict}  resolution: {resolution}\n"
        );
        let mutations = resolve(&support::draft(&yaml)).unwrap();
        mutations
            .iter()
            .find_map(|mutation| match mutation {
                Mutation::EditResource { resource, .. } => {
                    Some(cairn_schema::to_json(resource).unwrap())
                }
                _ => None,
            })
            .unwrap()
    };
    assert!(drafted("{remap_role: {role: r_kept}}").contains("{{roles.r_kept.name}}"));
    assert!(drafted("remove").contains("Hi Approver"));
}
