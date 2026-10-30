//! Save as route (B8): a journey saved as a route, published, and created again reproduces
//! its structure by key; each participation mapping; a retired key is named.
#![cfg(test)]

use crate::engine::support;

use cairn_engine::{Records, candidate, destination_after, preview, save_as_route};
use cairn_schema::{
    DestinationAfter, Domain, Graph, ParticipationMapping, ParticipationSource, ProposalDraft,
    ReviewItem,
};

const SAVED: &str = "vendor-saved";

fn journey_id() -> cairn_schema::JourneyId {
    "j_vendor_eval".parse().unwrap()
}

fn save(records: &Records) -> ProposalDraft {
    save_as_route(
        records,
        &journey_id(),
        &SAVED.parse().unwrap(),
        Some(&"Vendor evaluation, as run".parse().unwrap()),
        &support::fixed_inputs(),
    )
    .unwrap()
}

/// The draft with every participation mapped as `mapping` says.
fn mapped(draft: &ProposalDraft, mapping: &ParticipationMapping) -> ProposalDraft {
    let items = draft.items.as_slice().iter().map(|item| match item {
        ReviewItem::Participation { identity, uses, .. } => ReviewItem::Participation {
            identity: identity.clone(),
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
/// every node comes back with its key and structure, and none of its state.
#[test]
fn a_saved_journey_creates_its_structure_again() {
    let records = support::finished("vendor-evaluation");
    let draft = save(&records);
    let after = saved_and_created(&records, &draft, "j_again");
    let original = support::vendor_graph(&records);
    let again = &after.journeys[&"j_again".parse().unwrap()].graph;
    assert_eq!(again.nodes, original.nodes);
    assert_eq!(again.roles, original.roles);
    assert!(again.state.answers.is_empty() && again.state.pins.is_empty());
}

/// The graph the proposal's candidate leaves in its destination: a journey's, or a route's draft.
fn graph_after(records: &Records, destination: &Domain, draft: &ProposalDraft) -> Graph {
    let candidate = candidate(records, destination, draft, &support::fixed_inputs());
    let after = candidate.after.expect("a valid candidate");
    match destination_after(&after, destination, &support::derive_inputs_of(records)) {
        DestinationAfter::Journey { document } => document.journey.graph,
        DestinationAfter::Route { route, .. } => route.draft.expect("the draft it leaves").graph,
        other @ (DestinationAfter::Deployment { .. } | DestinationAfter::Absent) => {
            panic!("a graph destination, not {other:?}")
        }
    }
}

/// B8: each mapping choice for an explicit identity, as the preview of the saved route's draft
/// shows it: dropped, mapped to a role, to a new role, or to the default owner's role.
#[test]
fn each_participation_mapping() {
    let records = support::accepted_on(
        &support::vendor_after(2),
        "j_vendor_eval",
        "- op: add_node\n  node: {key: n_local, id: local, kind: action, title: Local, participations: {k_owner: [who_k3d7qm2xw5tn6pfa]}}\n",
    );
    let draft = save(&records);
    let blocked = preview(
        &records,
        &Domain::Route(SAVED.parse().unwrap()),
        &draft,
        &support::fixed_inputs(),
        &support::derive_inputs_of(&records),
    );
    assert_eq!(blocked.unresolved.len(), 1, "the identity needs a choice");
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
        let graph = graph_after(
            &records,
            &Domain::Route(SAVED.parse().unwrap()),
            &mapped(&draft, &mapping),
        );
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
        None,
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
    let latest = records
        .routes
        .get(&"vendor-evaluation".parse().unwrap())
        .and_then(|route| route.versions.iter().next_back().copied())
        .unwrap();
    assert!(
        violation.message.contains(&latest.to_string()),
        "the remedy names the latest version: {}",
        violation.message
    );
}
