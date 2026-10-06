//! Upgrade (B7): the three-way merge's outcomes, each conflict resolution applied, orphans
//! kept and removed, tombstones respected, and a violation surfacing as an item rather than
//! a silent apply.
#![cfg(test)]

mod support;

use cairn_engine::{Records, replay, upgrade};
use cairn_schema::{
    Conflict, ConflictResolution, Kept, LocalEdit, NodeField, ProposalDraft, Provenance,
    ReviewItem, VersionNumber, ViolationCode,
};

const JOURNEY: &str = "{journey: j_vendor_eval}";

fn journey_id() -> cairn_schema::JourneyId {
    "j_vendor_eval".parse().unwrap()
}

fn two() -> VersionNumber {
    VersionNumber::FIRST.next()
}

fn upgrade_to_two(records: &Records) -> ProposalDraft {
    upgrade(records, &journey_id(), two(), &support::fixed_inputs()).unwrap()
}

/// The draft with each conflict resolved by `choose`, and each orphan kept or not by `keep`.
fn chosen(
    draft: &ProposalDraft,
    choose: impl Fn(&Conflict) -> Option<ConflictResolution>,
    keep: bool,
) -> ProposalDraft {
    let items = draft
        .items
        .as_slice()
        .iter()
        .map(|item| match item {
            ReviewItem::Conflict {
                conflict,
                resolution,
            } => ReviewItem::Conflict {
                conflict: conflict.clone(),
                resolution: choose(conflict).or_else(|| resolution.clone()),
            },
            ReviewItem::Orphan { node, removal, .. } => ReviewItem::Orphan {
                node: node.clone(),
                keep,
                removal: removal.clone(),
            },
            other => other.clone(),
        })
        .collect();
    ProposalDraft {
        items: cairn_schema::BoundedVec::new(items).unwrap(),
        ..draft.clone()
    }
}

/// Proposes and applies the draft, panicking on a rejection; checks replay on the way (J3).
fn applied(records: &Records, draft: &ProposalDraft) -> Records {
    let proposed = support::propose(records, "pr_upgrade", JOURNEY, draft);
    let result = support::apply_proposal(&proposed, "pr_upgrade")
        .unwrap_or_else(|rejection| panic!("{rejection:#?}"));
    let after = result.records().clone();
    assert_eq!(
        replay(&proposed, result.events()),
        after,
        "J3: replay equals apply"
    );
    after
}

fn conflicts(draft: &ProposalDraft) -> Vec<&Conflict> {
    draft
        .items
        .as_slice()
        .iter()
        .filter_map(|item| match item {
            ReviewItem::Conflict { conflict, .. } => Some(conflict),
            _ => None,
        })
        .collect()
}

fn node<'a>(records: &'a Records, key: &str) -> &'a cairn_schema::Node<cairn_schema::KeyRefs> {
    support::vendor_graph(records)
        .nodes
        .get(&support::key(key))
        .unwrap()
}

/// B7 on the fixtures: the finished vendor evaluation upgraded to version 2 (fixtures/README.md)
/// proposes one orphan, the broken-down workload with its two journey-local children, and
/// nothing else; applied, the rename and the condition change land, the sign-off is added from
/// the route, and the workload stays, orphaned, with its state.
#[test]
fn version_two_of_the_vendor_evaluation() {
    let records = support::publish_vendor(
        &support::finished("vendor-evaluation"),
        support::vendor_v2(),
    );
    let draft = upgrade_to_two(&records);
    let [
        ReviewItem::Orphan {
            node: orphan,
            keep: true,
            removal,
        },
    ] = draft.items.as_slice()
    else {
        panic!("{:#?}", draft.items)
    };
    assert_eq!(orphan.as_str(), "n_workload");
    let removed: Vec<&str> = removal
        .descendants
        .iter()
        .map(cairn_schema::NodeKey::as_str)
        .collect();
    assert_eq!(removed, ["n_workload_ingest", "n_workload_query"]);

    let after = applied(&records, &draft);
    assert_eq!(
        node(&after, "n_access").title.as_str(),
        "Environment and data access"
    );
    let route_baseline = support::vendor_v2()
        .nodes
        .get(&support::key("n_baseline"))
        .unwrap()
        .relevant_when
        .clone();
    assert_eq!(node(&after, "n_baseline").relevant_when, route_baseline);
    let state = &support::vendor_graph(&after).state;
    assert_eq!(
        state.nodes[&support::key("n_signoff")].provenance,
        Provenance::FromRoute
    );
    assert_eq!(
        state.nodes[&support::key("n_workload")].provenance,
        Provenance::Orphaned
    );
    assert_eq!(
        state.nodes[&support::key("n_workload")].state,
        cairn_schema::State::Done
    );
    assert!(
        state.local_edits.is_empty(),
        "taking the route's values marks nothing"
    );
    let header = &after.journeys[&journey_id()].header;
    assert_eq!(header.lineage.as_ref().unwrap().version, two());
}

/// B7: removing the orphan removes its journey-local descendants with it.
#[test]
fn an_orphan_removed_takes_its_subtree() {
    let records = support::publish_vendor(
        &support::finished("vendor-evaluation"),
        support::vendor_v2(),
    );
    let after = applied(
        &records,
        &chosen(&upgrade_to_two(&records), |_| None, false),
    );
    let graph = support::vendor_graph(&after);
    for key in ["n_workload", "n_workload_ingest", "n_workload_query"] {
        assert!(graph.nodes.get(&support::key(key)).is_none(), "{key}");
    }
}

/// The vendor journey after its first step, with these local mutations, and version 2.
fn edited(mutations: &str) -> Records {
    let records = support::vendor_after(1);
    let records = if mutations.is_empty() {
        records
    } else {
        support::accepted_on(&records, "j_vendor_eval", mutations)
    };
    support::publish_vendor(&records, support::vendor_v2())
}

const RETITLE: &str =
    "- op: set_node_field\n  node: n_access\n  value: {title: Access to the environment}\n";

/// B7: an edit the route also changed differently is a conflict; keeping it leaves the
/// journey's value and its marker, taking the route's writes the route's and clears it.
#[test]
fn an_edit_both_sides_changed_is_kept_or_taken() {
    let records = edited(RETITLE);
    let draft = upgrade_to_two(&records);
    let [
        Conflict::Field {
            node,
            journey,
            route,
            ..
        },
    ] = conflicts(&draft)[..]
    else {
        panic!("{:#?}", draft.items)
    };
    assert_eq!(node.as_str(), "n_access");
    assert_ne!(journey, route);
    let unresolved = support::propose(&records, "pr_upgrade", JOURNEY, &draft);
    assert_eq!(
        support::codes(support::apply_proposal(&unresolved, "pr_upgrade")),
        [ViolationCode::UnresolvedReviewItem]
    );
    let title = LocalEdit::Field(NodeField::Title);
    let kept = applied(
        &records,
        &chosen(&draft, |_| Some(ConflictResolution::KeepJourney), true),
    );
    assert_eq!(node_title(&kept), "Access to the environment");
    assert!(
        support::vendor_graph(&kept).state.local_edits[&support::key("n_access")].contains(&title)
    );
    let taken = applied(
        &records,
        &chosen(&draft, |_| Some(ConflictResolution::TakeRoute), true),
    );
    assert_eq!(node_title(&taken), "Environment and data access");
    assert!(
        !support::vendor_graph(&taken)
            .state
            .local_edits
            .contains_key(&support::key("n_access"))
    );
}

fn node_title(records: &Records) -> &str {
    node(records, "n_access").title.as_str()
}

/// B7: an edit the route left alone is kept and listed; one the route made the same way
/// converges and its marker goes.
#[test]
fn kept_and_converged_edits() {
    let records = edited(
        "- op: set_node_field\n  node: n_findings\n  value: {estimate: 4}\n\
         - op: set_node_field\n  node: n_access\n  value: {title: Environment and data access}\n",
    );
    let draft = upgrade_to_two(&records);
    assert_eq!(conflicts(&draft).len(), 0);
    let kept: Vec<&Kept> = draft
        .items
        .as_slice()
        .iter()
        .filter_map(|item| match item {
            ReviewItem::KeptLocalEdit { kept } => Some(kept),
            _ => None,
        })
        .collect();
    assert_eq!(
        kept,
        [&Kept::Node {
            node: support::key("n_findings"),
            edit: LocalEdit::Field(NodeField::Estimate)
        }]
    );
    let after = applied(&records, &draft);
    let edits = &support::vendor_graph(&after).state.local_edits;
    assert!(edits.contains_key(&support::key("n_findings")));
    assert!(!edits.contains_key(&support::key("n_access")));
}

/// B4, B7: a route node the journey removed is not added back, nor is a new route node
/// under it, while its siblings still take the route's changes.
#[test]
fn tombstoned_nodes_are_not_re_added() {
    let records = support::accepted_on(
        &support::vendor_after(1),
        "j_vendor_eval",
        "- op: remove_node\n  removal: {node: n_partner_led, descendants: [n_criteria, n_partner_results], edges: [{node: n_partner_results, requires: n_criteria}]}\n",
    );
    let mut target = support::vendor_v2();
    let mut criteria = target
        .nodes
        .get(&support::key("n_criteria"))
        .unwrap()
        .clone();
    criteria.title = "Review the partner's criteria twice".parse().unwrap();
    target.nodes.put(criteria).unwrap();
    target
        .nodes
        .put(support::node(
            "{key: n_partner_extra, id: extra, parent: n_partner_led, kind: action, title: Extra}",
        ))
        .unwrap();
    let records = support::publish_vendor(&records, target);
    let after = applied(&records, &upgrade_to_two(&records));
    let graph = support::vendor_graph(&after);
    for key in [
        "n_partner_led",
        "n_criteria",
        "n_partner_results",
        "n_partner_extra",
    ] {
        assert!(
            graph.nodes.get(&support::key(key)).is_none(),
            "{key} is back"
        );
    }
    assert!(graph.nodes.get(&support::key("n_signoff")).is_some());
}

/// Version 2 with the purpose decision's `purchase` choice renamed `buy`, which the journey
/// answered.
fn without_purchase() -> cairn_schema::Graph {
    let mut target = support::vendor_v2();
    let mut purpose = target
        .nodes
        .get(&support::key("n_purpose"))
        .unwrap()
        .clone();
    let choices: cairn_schema::Choices =
        cairn_schema::from_yaml("[buy, {id: research-only, title: Research only}]").unwrap();
    assert!(cairn_schema::NodeFieldValue::Choices(choices).write(&mut purpose));
    target.nodes.put(purpose).unwrap();
    target
}

/// B7: a removed choice the journey answered is a conflict: mapped to a remaining choice,
/// kept as the journey's definition, or reopened.
#[test]
fn a_removed_answered_choice_maps_keeps_or_reopens() {
    let records = support::publish_vendor(&support::vendor_after(2), without_purchase());
    let draft = upgrade_to_two(&records);
    assert!(matches!(conflicts(&draft)[..], [Conflict::Answer { .. }]));
    let answer = |records: &Records| {
        support::vendor_graph(records)
            .state
            .answers
            .get(&support::key("n_purpose"))
            .cloned()
    };
    let map: std::collections::BTreeMap<cairn_schema::Slug, cairn_schema::Slug> =
        [("purchase".parse().unwrap(), "buy".parse().unwrap())].into();
    let mapped = applied(
        &records,
        &chosen(
            &draft,
            |_| Some(ConflictResolution::MapChoices { map: map.clone() }),
            true,
        ),
    );
    assert_eq!(
        answer(&mapped),
        Some(cairn_schema::from_yaml("single_choice: buy").unwrap())
    );
    let kept = applied(
        &records,
        &chosen(&draft, |_| Some(ConflictResolution::KeepJourney), true),
    );
    assert_eq!(answer(&kept), answer(&records));
    let reopened = applied(
        &records,
        &chosen(&draft, |_| Some(ConflictResolution::Reopen), true),
    );
    assert_eq!(answer(&reopened), None);
}

/// B7: a kind change under state (the access deliverable, started, becomes an action) is a
/// shape conflict; clearing the state takes the route's node, starting over in its kind.
#[test]
fn a_kind_change_under_state_clears_or_keeps() {
    let mut target = support::vendor_v2();
    target
        .nodes
        .put(support::node(
            "{key: n_access, id: access, parent: n_setup, kind: action, title: Environment and data access, estimate: 2}",
        ))
        .unwrap();
    let records = support::publish_vendor(&support::vendor_after(4), target);
    let draft = upgrade_to_two(&records);
    assert!(matches!(conflicts(&draft)[..], [Conflict::Shape { .. }]));
    let cleared = applied(
        &records,
        &chosen(&draft, |_| Some(ConflictResolution::ClearState), true),
    );
    assert_eq!(
        node(&cleared, "n_access").kind(),
        cairn_schema::NodeKind::Action
    );
    let state = &support::vendor_graph(&cleared).state;
    assert_eq!(
        state.nodes[&support::key("n_access")].state,
        cairn_schema::State::Todo
    );
    assert!(!state.local_edits.contains_key(&support::key("n_access")));
    let kept = applied(
        &records,
        &chosen(&draft, |_| Some(ConflictResolution::KeepJourney), true),
    );
    assert_eq!(
        node(&kept, "n_access").kind(),
        cairn_schema::NodeKind::Deliverable
    );
    let marked = &support::vendor_graph(&kept).state.local_edits[&support::key("n_access")];
    assert!(marked.contains(&LocalEdit::Shape));
}

/// Version 2 with the findings reviewer role removed, and its uses on route nodes with it.
fn without_findings_reviewer() -> cairn_schema::Graph {
    let mut target = support::vendor_v2();
    target.roles.remove(&"r_findings_reviewer".parse().unwrap());
    let mut decision = target
        .nodes
        .get(&support::key("n_findings_reviewer"))
        .unwrap()
        .clone();
    assert!(cairn_schema::NodeFieldValue::FillsRole(None).write(&mut decision));
    target.nodes.put(decision).unwrap();
    let mut report = target
        .nodes
        .get(&support::key("n_final_report"))
        .unwrap()
        .clone();
    report.participations = cairn_schema::Participations::default();
    target.nodes.put(report).unwrap();
    target
}

/// B7: a role the route removed that a journey-local node still uses is kept by default,
/// and can be remapped or removed with its references.
#[test]
fn a_removed_role_still_used_is_kept_remapped_or_removed() {
    let records = support::accepted_on(
        &support::vendor_after(1),
        "j_vendor_eval",
        "- op: add_node\n  node: {key: n_local, id: local, kind: action, title: Local, participations: {k_reviewer: r_findings_reviewer}}\n",
    );
    let records = support::publish_vendor(&records, without_findings_reviewer());
    let draft = upgrade_to_two(&records);
    let [
        Conflict::Role {
            route: None,
            references,
            ..
        },
    ] = conflicts(&draft)[..]
    else {
        panic!("{:#?}", draft.items)
    };
    assert_eq!(references.len(), 1, "only the local node still uses it");
    let has_role = |records: &Records| {
        support::vendor_graph(records)
            .roles
            .get(&"r_findings_reviewer".parse().unwrap())
            .is_some()
    };
    assert!(has_role(&applied(&records, &draft)), "kept by default");
    let remapped = applied(
        &records,
        &chosen(
            &draft,
            |_| {
                Some(ConflictResolution::RemapRole {
                    role: "r_eval_owner".parse().unwrap(),
                })
            },
            true,
        ),
    );
    assert!(!has_role(&remapped));
    let local = node(&remapped, "n_local")
        .participations
        .as_map()
        .values()
        .next()
        .cloned();
    assert_eq!(
        local,
        Some(cairn_schema::ParticipationSource::Role(
            "r_eval_owner".parse().unwrap()
        ))
    );
    let removed = applied(
        &records,
        &chosen(&draft, |_| Some(ConflictResolution::Remove), true),
    );
    assert!(!has_role(&removed));
    assert!(node(&removed, "n_local").participations.as_map().is_empty());
}

/// B7: a merge that breaks an invariant (the journey added its own `sign-off` beside the
/// route's) lists the violation, and applying the proposal as it stands is rejected for it.
#[test]
fn a_violation_is_an_item_never_a_silent_apply() {
    let records = edited(
        "- op: add_node\n  node: {key: n_own_signoff, id: sign-off, parent: n_reporting, kind: action, title: Our sign-off}\n",
    );
    let draft = upgrade_to_two(&records);
    let codes: Vec<ViolationCode> = draft
        .items
        .as_slice()
        .iter()
        .filter_map(|item| match item {
            ReviewItem::Violation { violation } => Some(violation.code),
            _ => None,
        })
        .collect();
    assert_eq!(codes, [ViolationCode::DuplicateSiblingId]);
    let proposed = support::propose(&records, "pr_upgrade", JOURNEY, &draft);
    assert_eq!(
        support::codes(support::apply_proposal(&proposed, "pr_upgrade")),
        [ViolationCode::DuplicateSiblingId]
    );
}

/// B7: upgrading to the version the journey follows proposes nothing, local edits or not.
#[test]
fn upgrading_to_the_same_version_proposes_nothing() {
    let records = support::accepted_on(&support::vendor_after(4), "j_vendor_eval", RETITLE);
    let draft = upgrade(
        &records,
        &journey_id(),
        VersionNumber::FIRST,
        &support::fixed_inputs(),
    )
    .unwrap();
    assert!(draft.mutations.is_empty() && draft.items.is_empty());
}

/// B7: the journey edited the reviewer participation of the final report, and the route
/// removed the reviewer kind with it: taking the route's side of the participation and
/// removing the kind write that participation once, and apply accepts them together.
#[test]
fn chosen_items_never_rewrite_one_participation_twice() {
    let records = support::accepted_on(
        &support::vendor_after(1),
        "j_vendor_eval",
        "- op: set_participation\n  node: n_final_report\n  kind: k_reviewer\n  source: r_eval_owner\n",
    );
    let mut target = support::vendor_v2();
    target
        .participation_kinds
        .remove(&"k_reviewer".parse().unwrap());
    let mut report = target
        .nodes
        .get(&support::key("n_final_report"))
        .unwrap()
        .clone();
    report.participations = cairn_schema::Participations::default();
    target.nodes.put(report).unwrap();
    let records = support::publish_vendor(&records, target);
    let draft = upgrade_to_two(&records);
    let both = chosen(
        &draft,
        |conflict| match conflict {
            Conflict::Participation { .. } => Some(ConflictResolution::TakeRoute),
            Conflict::Kind { route: None, .. } => Some(ConflictResolution::Remove),
            _ => None,
        },
        true,
    );
    let after = applied(&records, &both);
    let graph = support::vendor_graph(&after);
    assert!(
        graph
            .participation_kinds
            .get(&"k_reviewer".parse().unwrap())
            .is_none()
    );
    assert!(
        node(&after, "n_final_report")
            .participations
            .as_map()
            .is_empty()
    );
}

/// B7, A10: a role the route removed that a journey-local message draft still names is not
/// removed silently: it is a conflict, kept by default, and removing it writes the name out.
#[test]
fn a_role_a_draft_names_is_not_removed_silently() {
    let records = support::accepted_on(
        &support::vendor_after(1),
        "j_vendor_eval",
        "- op: add_node\n  node: {key: n_local, id: local, kind: action, title: Local, resources: [{key: a_msg, title: Msg, message_draft: 'Hi {{roles.r_findings_reviewer.name}}'}]}\n",
    );
    let records = support::publish_vendor(&records, without_findings_reviewer());
    let draft = upgrade_to_two(&records);
    assert!(matches!(
        conflicts(&draft)[..],
        [Conflict::Role { route: None, .. }]
    ));
    applied(&records, &draft);
    let removed = applied(
        &records,
        &chosen(&draft, |_| Some(ConflictResolution::Remove), true),
    );
    let resource = &node(&removed, "n_local").resources[0];
    let written = cairn_schema::to_json(resource).unwrap();
    assert!(written.contains("Hi Findings reviewer"), "{written}");
}

/// The journey after its first step with the partner-led group removed (tombstoned), and a
/// version 2 whose findings and a new node date themselves after a node inside that group.
fn dated_after_a_removed_node() -> Records {
    let records = support::accepted_on(
        &support::vendor_after(1),
        "j_vendor_eval",
        "- op: remove_node\n  removal: {node: n_partner_led, descendants: [n_criteria, n_partner_results], edges: [{node: n_partner_results, requires: n_criteria}]}\n",
    );
    let mut target = support::vendor_v2();
    let rule: cairn_schema::DateRule<cairn_schema::KeyRefs> =
        cairn_schema::from_yaml("{after: n_partner_results}").unwrap();
    let mut findings = target
        .nodes
        .get(&support::key("n_findings"))
        .unwrap()
        .clone();
    findings.not_before = Some(rule.clone());
    target.nodes.put(findings).unwrap();
    let mut extra = support::node(
        "{key: n_debrief, id: debrief, parent: n_reporting, kind: action, title: Debrief}",
    );
    extra.not_before = Some(rule);
    target.nodes.put(extra).unwrap();
    support::publish_vendor(&records, target)
}

/// B4, B7: a route value naming a node the journey removed is never applied silently: on a
/// shared node and on a new one it is a conflict, the journey's side (no rule) applies, and
/// keeping it marks the field so later upgrades keep it.
#[test]
fn a_route_value_naming_a_removed_node_is_a_conflict() {
    let records = dated_after_a_removed_node();
    let draft = upgrade_to_two(&records);
    let nodes: Vec<&str> = conflicts(&draft)
        .iter()
        .map(|conflict| match conflict {
            Conflict::Field { node, .. } => node.as_str(),
            other => panic!("{other:#?}"),
        })
        .collect();
    assert_eq!(nodes, ["n_findings", "n_debrief"]);
    let violations = draft
        .items
        .as_slice()
        .iter()
        .filter(|item| matches!(item, ReviewItem::Violation { .. }))
        .count();
    assert_eq!(violations, 0);
    let kept = applied(
        &records,
        &chosen(&draft, |_| Some(ConflictResolution::KeepJourney), true),
    );
    for key in ["n_findings", "n_debrief"] {
        assert_eq!(node(&kept, key).not_before, None, "{key}");
        let edits = &support::vendor_graph(&kept).state.local_edits[&support::key(key)];
        assert!(
            edits.contains(&LocalEdit::Field(NodeField::NotBefore)),
            "{key}"
        );
    }
}

/// Version 2 without the reviewer kind and the findings reviewer role, over a journey-local
/// node whose reviewer participation names that role.
fn role_and_kind_removed() -> Records {
    let records = support::accepted_on(
        &support::vendor_after(1),
        "j_vendor_eval",
        "- op: add_node\n  node: {key: n_local, id: local, kind: action, title: Local, participations: {k_reviewer: r_findings_reviewer}}\n",
    );
    let mut target = without_findings_reviewer();
    target
        .participation_kinds
        .remove(&"k_reviewer".parse().unwrap());
    support::publish_vendor(&records, target)
}

/// B7: a role item and a kind item over one participation never write it twice: every
/// combination of remapping and removing them applies, and a participation moved to another
/// kind carries the role item's choice.
#[test]
fn a_role_and_a_kind_over_one_participation_apply_together() {
    let records = role_and_kind_removed();
    let draft = upgrade_to_two(&records);
    let owner = || ConflictResolution::RemapRole {
        role: "r_eval_owner".parse().unwrap(),
    };
    let informed = || ConflictResolution::RemapKind {
        kind: "k_informed".parse().unwrap(),
    };
    let cases = [
        (ConflictResolution::Remove, ConflictResolution::Remove, None),
        (owner(), informed(), Some("r_eval_owner")),
        (ConflictResolution::Remove, informed(), None),
        (owner(), ConflictResolution::Remove, None),
    ];
    for (role, kind, moved) in cases {
        let both = chosen(
            &draft,
            |conflict| match conflict {
                Conflict::Role { route: None, .. } => Some(role.clone()),
                Conflict::Kind { route: None, .. } => Some(kind.clone()),
                _ => None,
            },
            true,
        );
        let after = applied(&records, &both);
        let participations = node(&after, "n_local").participations.as_map().clone();
        let expected: std::collections::BTreeMap<_, _> = moved
            .map(|role| {
                (
                    "k_informed".parse().unwrap(),
                    cairn_schema::ParticipationSource::Role(role.parse().unwrap()),
                )
            })
            .into_iter()
            .collect();
        assert_eq!(participations, expected, "{role:?} with {kind:?}");
    }
}

/// B7, A10: one message draft naming two roles the route removed is rewritten for both.
#[test]
fn a_draft_naming_two_removed_roles_is_rewritten_for_both() {
    let records = support::accepted_on(
        &support::vendor_after(1),
        "j_vendor_eval",
        "- op: add_node\n  node: {key: n_local, id: local, kind: action, title: Local, resources: [{key: a_msg, title: Msg, message_draft: 'Hi {{roles.r_findings_reviewer.name}} and {{roles.r_stakeholders.name}}'}]}\n",
    );
    let mut target = without_findings_reviewer();
    target.roles.remove(&"r_stakeholders".parse().unwrap());
    let mut informed = target
        .nodes
        .get(&support::key("n_who_informed"))
        .unwrap()
        .clone();
    assert!(cairn_schema::NodeFieldValue::FillsRole(None).write(&mut informed));
    target.nodes.put(informed).unwrap();
    let mut reporting = target
        .nodes
        .get(&support::key("n_reporting"))
        .unwrap()
        .clone();
    reporting.participations = cairn_schema::Participations::default();
    target.nodes.put(reporting).unwrap();
    let records = support::publish_vendor(&records, target);
    let draft = upgrade_to_two(&records);
    let removed = applied(
        &records,
        &chosen(&draft, |_| Some(ConflictResolution::Remove), true),
    );
    let written = cairn_schema::to_json(&node(&removed, "n_local").resources[0]).unwrap();
    assert!(
        written.contains("Hi Findings reviewer and Stakeholders"),
        "{written}"
    );
    let owner = ConflictResolution::RemapRole {
        role: "r_eval_owner".parse().unwrap(),
    };
    let remapped = applied(&records, &chosen(&draft, |_| Some(owner.clone()), true));
    let written = cairn_schema::to_json(&node(&remapped, "n_local").resources[0]).unwrap();
    assert_eq!(
        written.matches("{{roles.r_eval_owner.name}}").count(),
        2,
        "{written}"
    );
}

/// B4, B7, A10: a route message draft naming the answer of a node the journey removed is a
/// resource conflict offering only the journey's side; kept, the journey goes without it.
#[test]
fn a_route_draft_naming_a_removed_node_is_a_resource_conflict() {
    let records = support::accepted_on(
        &support::vendor_after(1),
        "j_vendor_eval",
        "- op: remove_node\n  removal: {node: n_partner_led, descendants: [n_criteria, n_partner_results], edges: [{node: n_partner_results, requires: n_criteria}]}\n",
    );
    let mut target = support::vendor_v2();
    target
        .nodes
        .put(support::node("{key: n_partner_ok, id: partner-ok, parent: n_partner_led, kind: decision, title: Partner OK, prompt: OK?, answer_type: boolean}"))
        .unwrap();
    let mut findings = target
        .nodes
        .get(&support::key("n_findings"))
        .unwrap()
        .clone();
    findings.resources.push(
        cairn_schema::from_yaml(
            "{key: a_note, title: Note, message_draft: 'Partner OK: {{answers.n_partner_ok}}'}",
        )
        .unwrap(),
    );
    target.nodes.put(findings).unwrap();
    let records = support::publish_vendor(&records, target);
    let draft = upgrade_to_two(&records);
    let [conflict @ Conflict::Resource { dangling: true, .. }] = conflicts(&draft)[..] else {
        panic!("{:#?}", draft.items)
    };
    assert!(
        !conflict.offers(&ConflictResolution::TakeRoute),
        "the route's draft cannot apply"
    );
    let kept = applied(
        &records,
        &chosen(&draft, |_| Some(ConflictResolution::KeepJourney), true),
    );
    assert_eq!(node(&kept, "n_findings").resources.len(), 0);
}

/// B7: a field conflict whose route value names a removed node offers only the journey's
/// side; taking the route's is reported as not offered.
#[test]
fn a_dangling_field_conflict_offers_only_the_journeys_side() {
    let records = dated_after_a_removed_node();
    let draft = upgrade_to_two(&records);
    for conflict in conflicts(&draft) {
        assert!(matches!(conflict, Conflict::Field { dangling: true, .. }));
        assert!(!conflict.offers(&ConflictResolution::TakeRoute));
    }
    let taking = chosen(&draft, |_| Some(ConflictResolution::TakeRoute), true);
    let reasons: Vec<_> = cairn_engine::resolve(&taking)
        .unwrap_err()
        .into_iter()
        .map(|item| item.reason)
        .collect();
    assert_eq!(reasons, [cairn_schema::UnresolvedReason::NotOffered; 2]);
}
