//! Save as route (B8): a journey's structure (nodes, edges, roles, kinds, conditions, rules,
//! weights, resources; no state, pins, answers, or notes) proposed as a new route's draft or
//! a new draft of an existing route, keys preserved. The review maps each explicit entity to a
//! role, drops it, or makes it the default owner, and may exclude nodes with their subtrees;
//! children a breakdown of a placeholder created start excluded.
//!
//! Cost: one pass over the nodes and their participations, O(n log n), plus the trial apply
//! that validates the draft once.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    Domain, DraftSource, EntityKey, JourneyId, Mutation, ParticipationRef, ParticipationSource,
    Payload, ProposalDraft, Provenance, ReviewItem, Revision, RouteId, Title,
};

use super::{DraftError, draft, trial, with_violations};
use crate::pipeline::ApplyInputs;
use crate::records::Records;

/// B8: the proposal that saves a journey as a draft of `route`: created with `name` when
/// the records hold no such route, or a new draft of it (A11: rejected at apply while
/// another draft is open). Every violation of a trial apply, with every entity dropped and
/// nothing more excluded, is listed.
///
/// # Errors
///
/// When the journey is not loaded, or the draft passes a proposal's limits.
pub fn save_as_route(
    records: &Records,
    journey: &JourneyId,
    route: &RouteId,
    name: &Title,
    inputs: &ApplyInputs,
) -> Result<ProposalDraft, DraftError> {
    let held = records
        .journeys
        .get(journey)
        .ok_or_else(|| DraftError::JourneyMissing(journey.clone()))?;
    let mut structure = held.graph.clone();
    structure.state = cairn_schema::JourneyState::default();
    let existing = records.routes.get(route);
    let mut mutations = Vec::new();
    if existing.is_none() {
        mutations.push(Mutation::CreateRoute {
            name: name.clone(),
            description: held.header.description.clone(),
        });
    }
    mutations.push(Mutation::OpenDraft {
        source: DraftSource::SaveAsRoute {
            journey: journey.clone(),
        },
    });
    mutations.extend(crate::format::content(&structure));
    let mut items = participation_items(&structure);
    items.extend(exclusion_items(&held.graph));
    let revision = existing.map_or(Revision::NONE, |found| found.revision);
    let title = format!("Save {} as a route", held.header.name.as_str());
    let proposal = draft(&title, revision, mutations, items)?;
    let dropped = everything_dropped(&proposal);
    let mut violations = retired_in_route(records, route, &structure);
    let explained: BTreeSet<_> = violations
        .iter()
        .map(|found| found.at.subject.clone())
        .collect();
    // The trial finds the same retired keys; each is listed once, with its remedy.
    let found = trial(records, &Domain::Route(route.clone()), &dropped, inputs);
    violations.extend(found.into_iter().filter(|found| {
        found.code != cairn_schema::ViolationCode::RetiredKeyReused
            || !explained.contains(&found.at.subject)
    }));
    with_violations(proposal, violations)
}

/// Invariants (no key is ever reused): the journey's roles, kinds, and nodes whose keys the
/// route's latest version retired (a journey behind its route) cannot come back, and only a
/// node can be excluded; each is listed with why and what to do.
fn retired_in_route(
    records: &Records,
    route: &RouteId,
    structure: &cairn_schema::Graph,
) -> Vec<cairn_schema::Violation> {
    let latest = records.routes.get(route).and_then(|found| {
        let version = *found.versions.iter().next_back()?;
        let lineage = cairn_schema::Lineage {
            route: route.clone(),
            version,
        };
        records
            .versions
            .get(&lineage)
            .map(|held| (version, &held.graph.retired_keys))
    });
    let Some((version, retired)) = latest else {
        return Vec::new();
    };
    let remedy = format!(
        "version {version} of route {route} retired it; upgrade the journey to version {version} first, or save it as a new route"
    );
    let subjects = structure
        .roles
        .as_map()
        .keys()
        .filter(|key| retired.roles.contains(*key))
        .map(|key| {
            (
                cairn_schema::Subject::Role(key.clone()),
                format!("role {key}"),
            )
        })
        .chain(
            structure
                .participation_kinds
                .as_map()
                .keys()
                .filter(|key| retired.kinds.contains(*key))
                .map(|key| {
                    (
                        cairn_schema::Subject::Kind(key.clone()),
                        format!("participation kind {key}"),
                    )
                }),
        )
        .chain(
            structure
                .nodes
                .as_map()
                .keys()
                .filter(|key| retired.nodes.contains(*key))
                .map(|key| {
                    (
                        cairn_schema::Subject::Node(key.clone()),
                        format!("node {key} (or exclude it)"),
                    )
                }),
        );
    subjects
        .map(|(subject, what)| {
            let mut found = crate::validate::violation(
                cairn_schema::ViolationCode::RetiredKeyReused,
                format!("{what} cannot be saved: {remedy}"),
            );
            found.at.subject = Some(subject);
            found
        })
        .collect()
}

/// B8: one item per explicit entity, naming every participation that names it.
fn participation_items(structure: &cairn_schema::Graph) -> Vec<ReviewItem> {
    let mut uses: BTreeMap<EntityKey, BTreeSet<ParticipationRef>> = BTreeMap::new();
    for node in structure.nodes.values() {
        for (kind, source) in node.participations.as_map() {
            let ParticipationSource::Entities(entities) = source else {
                continue;
            };
            for entity in entities.iter() {
                uses.entry(entity.clone())
                    .or_default()
                    .insert(ParticipationRef {
                        node: node.key.clone(),
                        kind: kind.clone(),
                    });
            }
        }
    }
    uses.into_iter()
        .map(|(entity, uses)| ReviewItem::Participation {
            entity,
            uses,
            mapping: None,
        })
        .collect()
}

/// B8: one item per node; a journey-local node under a placeholder (a breakdown's child)
/// starts excluded.
fn exclusion_items(journey: &cairn_schema::Graph) -> Vec<ReviewItem> {
    let placeholder = |key: &cairn_schema::NodeKey| {
        journey
            .nodes
            .get(key)
            .is_some_and(|node| match &node.payload {
                Payload::Deliverable(deliverable) => deliverable.placeholder,
                Payload::Action(action) => action.placeholder,
                Payload::Decision(_) | Payload::Milestone(_) | Payload::Group(_) => false,
            })
    };
    journey
        .nodes
        .values()
        .map(|node| {
            let local = journey
                .state
                .nodes
                .get(&node.key)
                .is_some_and(|state| state.provenance == Provenance::Local);
            ReviewItem::Exclusion {
                node: node.key.clone(),
                excluded: local && node.parent.as_ref().is_some_and(placeholder),
            }
        })
        .collect()
}

/// The proposal with every entity dropped, for the trial apply before anyone has chosen.
fn everything_dropped(proposal: &ProposalDraft) -> ProposalDraft {
    let items = proposal.items.as_slice().iter().map(|item| match item {
        ReviewItem::Participation { entity, uses, .. } => ReviewItem::Participation {
            entity: entity.clone(),
            uses: uses.clone(),
            mapping: Some(cairn_schema::ParticipationMapping::Drop),
        },
        other => other.clone(),
    });
    match cairn_schema::BoundedVec::new(items.collect()) {
        Ok(items) => ProposalDraft {
            items,
            ..proposal.clone()
        },
        Err(error) => unreachable!("the same number of items fits: {error}"),
    }
}
