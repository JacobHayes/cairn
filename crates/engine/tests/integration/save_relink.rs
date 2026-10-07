//! Save as route (B8), re-link (B9), and previews (C14): a journey saved as a route,
//! published, and created again reproduces its structure by key; each participation mapping;
//! re-linking keeps differences as local edits unless the route's value is taken; a preview
//! shows the candidate, its frontier, and what is unresolved.
#![cfg(test)]

use crate::support;

use cairn_engine::{Records, preview, relink, save_as_route, upgrade};
use cairn_schema::{
    ConflictResolution, Domain, Lineage, LocalEdit, NodeField, ParticipationMapping,
    ParticipationSource, ProposalDraft, Provenance, ReviewItem, VersionNumber,
};

const SAVED: &str = "vendor-saved";

fn journey_id() -> cairn_schema::JourneyId {
    "j_vendor_eval".parse().unwrap()
}

fn saved_lineage() -> Lineage {
    Lineage {
        route: SAVED.parse().unwrap(),
        version: VersionNumber::FIRST,
    }
}

fn save(records: &Records) -> ProposalDraft {
    save_as_route(
        records,
        &journey_id(),
        &SAVED.parse().unwrap(),
        &"Vendor evaluation, as run".parse().unwrap(),
        &support::fixed_inputs(),
    )
    .unwrap()
}

/// The draft with every participation mapped as `mapping` says.
fn mapped(draft: &ProposalDraft, mapping: &ParticipationMapping) -> ProposalDraft {
    let items = draft.items.as_slice().iter().map(|item| match item {
        ReviewItem::Participation { entity, uses, .. } => ReviewItem::Participation {
            entity: entity.clone(),
            uses: uses.clone(),
            mapping: Some(mapping.clone()),
        },
        other => other.clone(),
    });
    ProposalDraft {
        items: cairn_schema::BoundedVec::new(items.collect()).unwrap(),
        ..draft.clone()
    }
}

/// Applies the saved route's proposal, publishes its draft, and creates `journey` from it.
fn saved_and_created(records: &Records, draft: &ProposalDraft, journey: &str) -> Records {
    let destination = format!("{{route: {SAVED}}}");
    let proposed = support::propose(records, "pr_save", &destination, draft);
    let applied = support::apply_proposal(&proposed, "pr_save")
        .unwrap_or_else(|rejection| panic!("{rejection:#?}"));
    let published = support::accepted_to(applied.records(), &destination, "- op: publish_draft\n");
    support::accepted_to(
        &published,
        &format!("{{journey: {journey}}}"),
        &format!("- op: create_journey\n  name: Again\n  from: {{route: {SAVED}, version: 1}}\n"),
    )
}

/// B8, B1: the finished vendor evaluation saved as a route, published, and created again:
/// every node but the breakdown's children (excluded by default) comes back with its key
/// and structure, and none of its state.
#[test]
fn a_saved_journey_creates_its_structure_again() {
    let records = support::finished("vendor-evaluation");
    let draft = save(&records);
    let excluded: Vec<&str> = draft
        .items
        .as_slice()
        .iter()
        .filter_map(|item| match item {
            ReviewItem::Exclusion {
                node,
                excluded: true,
            } => Some(node.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(excluded, ["n_workload_ingest", "n_workload_query"]);
    let after = saved_and_created(&records, &draft, "j_again");
    let original = support::vendor_graph(&records);
    let again = &after.journeys[&"j_again".parse().unwrap()].graph;
    let mut expected = original.nodes.clone();
    for key in excluded {
        expected.remove(&support::key(key));
    }
    assert_eq!(again.nodes, expected);
    assert_eq!(again.roles, original.roles);
    assert!(again.state.answers.is_empty() && again.state.pins.is_empty());
}

/// B9: re-linking the saved journey to the version it published matches every included node
/// by key with nothing to keep or take, so it marks nothing; the excluded children stay the
/// journey's own; an upgrade to that version then proposes nothing.
#[test]
fn re_linking_after_a_save_establishes_the_base() {
    let records = support::finished("vendor-evaluation");
    let after = saved_and_created(&records, &save(&records), "j_again");
    let draft = relink(&after, &journey_id(), &saved_lineage()).unwrap();
    assert!(draft.items.is_empty(), "{:#?}", draft.items);
    let proposed = support::propose(&after, "pr_relink", "{journey: j_vendor_eval}", &draft);
    let linked = support::apply_proposal(&proposed, "pr_relink").unwrap();
    let journey = &linked.records().journeys[&journey_id()];
    assert_eq!(journey.header.lineage, Some(saved_lineage()));
    let state = &journey.graph.state;
    assert!(state.local_edits.is_empty());
    assert_eq!(
        state.nodes[&support::key("n_workload_query")].provenance,
        Provenance::Local
    );
    assert_eq!(
        state.nodes[&support::key("n_access")].provenance,
        Provenance::FromRoute
    );
    let again = upgrade(
        linked.records(),
        &journey_id(),
        VersionNumber::FIRST,
        &support::fixed_inputs(),
    )
    .unwrap();
    assert!(again.items.is_empty());
}

/// B9: a field the journey changed after the save differs from the version: kept as a local
/// edit by default, or set to the route's value with no marker.
#[test]
fn a_difference_is_kept_unless_the_route_value_is_taken() {
    let records = support::finished("vendor-evaluation");
    let after = saved_and_created(&records, &save(&records), "j_again");
    let after = support::accepted_on(
        &after,
        "j_vendor_eval",
        "- op: set_node_field\n  node: n_access\n  value: {title: Access changed after the save}\n",
    );
    let draft = relink(&after, &journey_id(), &saved_lineage()).unwrap();
    let [
        ReviewItem::Conflict {
            resolution: Some(ConflictResolution::KeepJourney),
            ..
        },
    ] = draft.items.as_slice()
    else {
        panic!("{:#?}", draft.items)
    };
    let title = LocalEdit::Field(NodeField::Title);
    let linked = |draft: &ProposalDraft| {
        let proposed = support::propose(&after, "pr_relink", "{journey: j_vendor_eval}", draft);
        support::apply_proposal(&proposed, "pr_relink")
            .unwrap()
            .records()
            .clone()
    };
    let kept = linked(&draft);
    let edits = &support::vendor_graph(&kept).state.local_edits;
    assert_eq!(
        edits.get(&support::key("n_access")),
        Some(&[title.clone()].into())
    );
    let taking = ProposalDraft {
        items: cairn_schema::BoundedVec::new(vec![match &draft.items.as_slice()[0] {
            ReviewItem::Conflict { conflict, .. } => ReviewItem::Conflict {
                conflict: conflict.clone(),
                resolution: Some(ConflictResolution::TakeRoute),
            },
            other => other.clone(),
        }])
        .unwrap(),
        ..draft.clone()
    };
    let taken = linked(&taking);
    let graph = support::vendor_graph(&taken);
    assert_eq!(
        graph
            .nodes
            .get(&support::key("n_access"))
            .unwrap()
            .title
            .as_str(),
        "Environment access"
    );
    assert!(graph.state.local_edits.is_empty());
}

/// B8: each mapping choice for an explicit entity, as the preview of the saved route's draft
/// shows it: dropped, mapped to a role, to a new role, or to the default owner's role.
#[test]
fn each_participation_mapping() {
    let records = support::accepted_on(
        &support::vendor_after(2),
        "j_vendor_eval",
        "- op: add_node\n  node: {key: n_local, id: local, kind: action, title: Local, participations: {k_owner: [e_lead]}}\n",
    );
    let draft = save(&records);
    let blocked = preview(
        &records,
        &Domain::Route(SAVED.parse().unwrap()),
        &draft,
        &support::fixed_inputs(),
        &support::derive_inputs_of(&records),
    );
    assert_eq!(blocked.unresolved.len(), 1, "the entity needs a choice");
    let lead = || "r_lead".parse().unwrap();
    let cases = [
        (ParticipationMapping::Drop, None),
        (
            ParticipationMapping::Role("r_eval_owner".parse().unwrap()),
            Some("r_eval_owner"),
        ),
        (
            ParticipationMapping::NewRole(cairn_schema::Role {
                key: lead(),
                id: "lead".parse().unwrap(),
                title: None,
                multi: false,
            }),
            Some("r_lead"),
        ),
        (ParticipationMapping::DefaultOwner, Some("r_eval_owner")),
    ];
    for (mapping, role) in cases {
        let shown = preview(
            &records,
            &Domain::Route(SAVED.parse().unwrap()),
            &mapped(&draft, &mapping),
            &support::fixed_inputs(),
            &support::derive_inputs_of(&records),
        );
        assert!(
            shown.unresolved.is_empty() && shown.violations.is_empty(),
            "{mapping:?}: {shown:#?}"
        );
        let graph = shown.graph.unwrap();
        let owner = graph
            .nodes
            .get(&support::key("n_local"))
            .unwrap()
            .participations
            .as_map()
            .values()
            .next()
            .cloned();
        assert_eq!(
            owner,
            role.map(|role| ParticipationSource::Role(role.parse().unwrap())),
            "{mapping:?}"
        );
    }
}

/// C14, D7: an upgrade's preview over the journey: the candidate graph, its frontier, and
/// the consequences, with an unresolved conflict listed and left at the journey's side.
#[test]
fn an_upgrade_preview_shows_the_journey_after() {
    let records = support::accepted_on(
        &support::vendor_after(3),
        "j_vendor_eval",
        "- op: set_node_field\n  node: n_access\n  value: {title: Access to the environment}\n",
    );
    let records = support::publish_vendor(&records, support::vendor_v2());
    let draft = upgrade(
        &records,
        &journey_id(),
        VersionNumber::FIRST.next(),
        &support::fixed_inputs(),
    )
    .unwrap();
    let shown = preview(
        &records,
        &Domain::Journey(journey_id()),
        &draft,
        &support::fixed_inputs(),
        &support::derive_inputs_of(&records),
    );
    assert_eq!(shown.unresolved.len(), 1);
    assert_eq!(shown.violations.len(), 0);
    let graph = shown.graph.unwrap();
    assert_eq!(
        graph
            .nodes
            .get(&support::key("n_access"))
            .unwrap()
            .title
            .as_str(),
        "Access to the environment"
    );
    assert!(graph.nodes.get(&support::key("n_signoff")).is_some());
    assert!(shown.frontier.contains(&support::key("n_access")));
    assert!(shown.consequences.is_some());
}

/// Invariants, B8: a journey behind its route that still uses a role the latest version
/// retired cannot be saved as the route's next draft; the proposal says which key, why, and
/// to upgrade the journey first.
#[test]
fn saving_a_journey_behind_its_route_names_the_retired_keys() {
    let mut target = support::vendor_v2();
    target.roles.remove(&"r_findings_reviewer".parse().unwrap());
    target
        .retired_keys
        .roles
        .insert("r_findings_reviewer".parse().unwrap());
    let records = support::publish_vendor(&support::vendor_after(1), target);
    let draft = save_as_route(
        &records,
        &journey_id(),
        &"vendor-evaluation".parse().unwrap(),
        &"Vendor evaluation".parse().unwrap(),
        &support::fixed_inputs(),
    )
    .unwrap();
    let explained: Vec<&cairn_schema::Violation> = draft
        .items
        .as_slice()
        .iter()
        .filter_map(|item| match item {
            ReviewItem::Violation { violation }
                if violation.at.subject
                    == Some(cairn_schema::Subject::Role(
                        "r_findings_reviewer".parse().unwrap(),
                    )) =>
            {
                Some(violation)
            }
            _ => None,
        })
        .collect();
    let [violation] = explained[..] else {
        panic!("{:#?}", draft.items)
    };
    assert_eq!(
        violation.code,
        cairn_schema::ViolationCode::RetiredKeyReused
    );
}
