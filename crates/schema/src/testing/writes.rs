//! Strategies for patches, mutations, proposals, records, events, and change sets.

use proptest::prelude::*;

use super::model::bounded_vec;
use super::{
    arb_agent_id, arb_annotation, arb_annotation_body, arb_answer_value, arb_attachment_key,
    arb_choices, arb_date, arb_entity, arb_entity_key, arb_entity_set, arb_guard, arb_insertion,
    arb_insertion_key, arb_journey_header, arb_journey_id, arb_journey_status, arb_kind_key,
    arb_lineage, arb_local_edit, arb_markdown, arb_node, arb_node_field_value, arb_node_key,
    arb_node_state, arb_overrides, arb_participation_kind, arb_patch_id, arb_proposal_id,
    arb_reason, arb_resource, arb_revision, arb_role, arb_role_key, arb_route_header, arb_route_id,
    arb_route_kind, arb_slug, arb_snooze_target, arb_timestamp, arb_title, arb_user_id,
    arb_version_number,
};
use crate::domain::{Domain, GraphId};
use crate::event::{Actor, ChangeSet, Event, PatchReceipt, Subject};
use crate::graph::Edge;
use crate::node::ParticipationSource;
use crate::number::SignedDays;
use crate::patch::{
    DraftSource, EdgeEnd, InsertedEdge, KindChoice, Mutation, Mutations, Override,
    ParticipationRef, Patch, PatchTarget, RecordedEnd, Removal, RoleChoice, Transition,
};
use crate::proposal::{
    Conflict, ConflictResolution, Kept, ParticipationMapping, Proposal, ProposalDraft,
    ProposalStatus, ReviewItem, RoleReference,
};
use crate::record::{GraphRecord, Record, RecordKey, RetiredKey, Write};
use crate::refs::KeyRefs;
use crate::state::{OverrideKind, Provenance};

/// A domain.
pub fn arb_domain() -> BoxedStrategy<Domain> {
    prop_oneof![
        arb_journey_id().prop_map(Domain::Journey),
        arb_route_id().prop_map(Domain::Route),
        Just(Domain::Deployment),
    ]
    .boxed()
}

/// A graph id.
pub fn arb_graph_id() -> BoxedStrategy<GraphId> {
    prop_oneof![
        arb_journey_id().prop_map(GraphId::Journey),
        arb_route_id().prop_map(GraphId::RouteDraft),
        (arb_route_id(), arb_version_number())
            .prop_map(|(route, version)| GraphId::RouteVersion { route, version }),
    ]
    .boxed()
}

/// An explicit edge.
pub fn arb_edge() -> BoxedStrategy<Edge> {
    (arb_node_key(), arb_node_key())
        .prop_map(|(node, requires)| Edge { node, requires })
        .boxed()
}

/// A node removal.
pub fn arb_removal() -> BoxedStrategy<Removal> {
    (
        arb_node_key(),
        prop::collection::btree_set(arb_node_key(), 0..3),
        prop::collection::btree_set(arb_edge(), 0..2),
        prop::collection::btree_set(arb_attachment_key(), 0..2),
        prop::collection::btree_set(arb_attachment_key(), 0..2),
        prop::collection::btree_set(
            (arb_node_key(), arb_kind_key())
                .prop_map(|(node, kind)| ParticipationRef { node, kind }),
            0..2,
        ),
    )
        .prop_map(
            |(node, descendants, edges, resources, annotations, participations)| Removal {
                node,
                descendants,
                edges,
                resources,
                annotations,
                participations,
            },
        )
        .boxed()
}

fn arb_transition() -> BoxedStrategy<Transition> {
    prop_oneof![
        Just(Transition::Start),
        Just(Transition::Stop),
        Just(Transition::Complete),
        arb_reason().prop_map(|reason| Transition::Skip { reason }),
        Just(Transition::Reopen),
        Just(Transition::Reach),
    ]
    .boxed()
}

fn arb_override() -> BoxedStrategy<Override> {
    prop_oneof![
        arb_reason().prop_map(|reason| Override::ForceInclude { reason }),
        arb_reason().prop_map(|reason| Override::Keep { reason }),
        (prop::collection::btree_set(arb_guard(), 1..3), arb_reason())
            .prop_map(|(guards, reason)| Override::GuardBypass { guards, reason }),
    ]
    .boxed()
}

fn arb_lifecycle_mutation() -> BoxedStrategy<Mutation> {
    prop_oneof![
        (
            arb_title(),
            prop::option::of(arb_markdown()),
            prop::option::of(arb_lineage())
        )
            .prop_map(|(name, description, from)| Mutation::CreateJourney {
                name,
                description,
                from
            }),
        (arb_title(), prop::option::of(arb_markdown()))
            .prop_map(|(name, description)| Mutation::EditJourney { name, description }),
        arb_journey_status().prop_map(|status| Mutation::SetJourneyStatus { status }),
        Just(Mutation::DeleteJourney {}),
        (
            arb_title(),
            prop::option::of(arb_markdown()),
            arb_route_kind()
        )
            .prop_map(|(name, description, kind)| Mutation::CreateRoute {
                name,
                description,
                kind
            }),
        (arb_title(), prop::option::of(arb_markdown()))
            .prop_map(|(name, description)| Mutation::EditRoute { name, description }),
        any::<bool>().prop_map(|retired| Mutation::SetRouteRetired { retired }),
        prop_oneof![
            Just(DraftSource::Edit),
            Just(DraftSource::Import),
            arb_journey_id().prop_map(|journey| DraftSource::SaveAsRoute { journey }),
        ]
        .prop_map(|source| Mutation::OpenDraft { source }),
        Just(Mutation::DiscardDraft {}),
        Just(Mutation::PublishDraft {}),
    ]
    .boxed()
}

fn arb_structure_mutation() -> BoxedStrategy<Mutation> {
    let source = prop_oneof![
        arb_role_key().prop_map(ParticipationSource::Role),
        arb_entity_set().prop_map(ParticipationSource::Entities),
    ];
    prop_oneof![
        arb_node::<KeyRefs>().prop_map(|node| Mutation::AddNode { node }),
        (arb_node_key(), arb_node_field_value::<KeyRefs>())
            .prop_map(|(node, value)| Mutation::SetNodeField { node, value }),
        arb_node::<KeyRefs>().prop_map(|node| Mutation::ReplaceNode { node }),
        arb_removal().prop_map(|removal| Mutation::RemoveNode { removal }),
        arb_edge().prop_map(|edge| Mutation::AddEdge { edge }),
        arb_edge().prop_map(|edge| Mutation::RemoveEdge { edge }),
        arb_role::<KeyRefs>().prop_map(|role| Mutation::AddRole { role }),
        arb_role::<KeyRefs>().prop_map(|role| Mutation::EditRole { role }),
        arb_role_key().prop_map(|role| Mutation::RemoveRole { role }),
        arb_participation_kind::<KeyRefs>()
            .prop_map(|kind| Mutation::AddParticipationKind { kind }),
        arb_participation_kind::<KeyRefs>()
            .prop_map(|kind| Mutation::EditParticipationKind { kind }),
        arb_kind_key().prop_map(|kind| Mutation::RemoveParticipationKind { kind }),
        prop::option::of(arb_role_key()).prop_map(|role| Mutation::SetDefaultOwner { role }),
        (arb_node_key(), arb_kind_key(), source)
            .prop_map(|(node, kind, source)| Mutation::SetParticipation { node, kind, source }),
        (arb_node_key(), arb_kind_key())
            .prop_map(|(node, kind)| Mutation::ClearParticipation { node, kind }),
        (arb_node_key(), arb_resource::<KeyRefs>())
            .prop_map(|(node, resource)| Mutation::AddResource { node, resource }),
        (arb_node_key(), arb_resource::<KeyRefs>())
            .prop_map(|(node, resource)| Mutation::EditResource { node, resource }),
        (arb_node_key(), arb_attachment_key())
            .prop_map(|(node, resource)| Mutation::RemoveResource { node, resource }),
        arb_insert_segment(),
    ]
    .boxed()
}

fn arb_insert_segment() -> BoxedStrategy<Mutation> {
    let end = prop_oneof![
        arb_node_key().prop_map(EdgeEnd::Segment),
        arb_node_key().prop_map(EdgeEnd::Host),
    ];
    let edge = (end.clone(), end).prop_map(|(node, requires)| InsertedEdge { node, requires });
    let role = prop_oneof![
        arb_role_key().prop_map(RoleChoice::Existing),
        Just(RoleChoice::Add)
    ];
    let kind = prop_oneof![
        arb_kind_key().prop_map(KindChoice::Existing),
        Just(KindChoice::Add)
    ];
    (
        (
            arb_insertion_key(),
            arb_lineage(),
            prop::option::of(arb_node_key()),
            prop::option::of(arb_slug()),
            prop::option::of(arb_title()),
        ),
        prop::collection::btree_map(arb_role_key(), role, 0..3),
        prop::collection::btree_map(arb_kind_key(), kind, 0..3),
        prop::collection::btree_set(arb_node_key(), 0..3),
        prop::collection::vec(edge, 0..3),
    )
        .prop_map(
            |((insertion, segment, parent, root_id, root_title), roles, kinds, omit, edges)| {
                Mutation::InsertSegment {
                    insertion,
                    segment,
                    parent,
                    root_id,
                    root_title,
                    roles,
                    kinds,
                    omit,
                    edges,
                }
            },
        )
        .boxed()
}

fn arb_state_mutation() -> BoxedStrategy<Mutation> {
    let override_kind = prop::sample::select(vec![
        OverrideKind::ForceInclude,
        OverrideKind::Keep,
        OverrideKind::GuardBypass,
    ]);
    let shift =
        (-365i32..=365).prop_filter_map("within the limit", |days| SignedDays::try_from(days).ok());
    prop_oneof![
        (arb_node_key(), arb_transition())
            .prop_map(|(node, transition)| Mutation::Transition { node, transition }),
        (
            arb_node_key(),
            arb_answer_value(),
            prop::option::of(arb_markdown())
        )
            .prop_map(|(decision, value, rationale)| Mutation::Answer {
                decision,
                value,
                rationale
            }),
        (
            arb_node_key(),
            prop::sample::select(vec![RecordedEnd::Start, RecordedEnd::Finish]),
            arb_date()
        )
            .prop_map(|(node, end, date)| Mutation::SetRecordedDate { node, end, date }),
        (arb_role_key(), arb_entity_set())
            .prop_map(|(role, entities)| Mutation::FillRole { role, entities }),
        arb_role_key().prop_map(|role| Mutation::ClearRoleFill { role }),
        (arb_node_key(), arb_date()).prop_map(|(node, date)| Mutation::SetPin { node, date }),
        (arb_node_key(), shift)
            .prop_map(|(node, offset_days)| Mutation::ShiftPin { node, offset_days }),
        arb_node_key().prop_map(|node| Mutation::ClearPin { node }),
        (arb_node_key(), arb_snooze_target())
            .prop_map(|(node, until)| Mutation::Snooze { node, until }),
        arb_node_key().prop_map(|node| Mutation::Unsnooze { node }),
        (arb_node_key(), arb_override())
            .prop_map(|(node, applied)| Mutation::ApplyOverride { node, applied }),
        (arb_node_key(), override_kind)
            .prop_map(|(node, kind)| Mutation::RemoveOverride { node, kind }),
        (arb_node_key(), any::<bool>())
            .prop_map(|(node, atomic)| Mutation::SetAtomic { node, atomic }),
        arb_annotation_body().prop_map(|annotation| Mutation::AddAnnotation { annotation }),
        arb_annotation_body().prop_map(|annotation| Mutation::EditAnnotation { annotation }),
        arb_attachment_key().prop_map(|annotation| Mutation::RemoveAnnotation { annotation }),
    ]
    .boxed()
}

fn arb_lineage_and_entity_mutation() -> BoxedStrategy<Mutation> {
    let provenance = prop::sample::select(vec![
        Provenance::FromRoute,
        Provenance::Local,
        Provenance::Orphaned,
        Provenance::FromSegment,
    ]);
    prop_oneof![
        arb_version_number().prop_map(|to| Mutation::Upgrade { to }),
        arb_lineage().prop_map(|lineage| Mutation::Relink { lineage }),
        (arb_node_key(), provenance)
            .prop_map(|(node, provenance)| Mutation::SetProvenance { node, provenance }),
        (arb_node_key(), arb_local_edit(), any::<bool>())
            .prop_map(|(node, edit, marked)| Mutation::MarkLocalEdit { node, edit, marked }),
        arb_entity().prop_map(|entity| Mutation::CreateEntity { entity }),
        arb_entity().prop_map(|entity| Mutation::EditEntity { entity }),
        (
            arb_entity_key(),
            arb_entity_key(),
            prop::collection::btree_map(arb_journey_id(), arb_revision(), 0..2)
        )
            .prop_map(|(survivor, merged, journeys)| Mutation::MergeEntities {
                survivor,
                merged,
                journeys
            }),
    ]
    .boxed()
}

/// A mutation that does not edit a proposal: what a proposal may hold.
pub fn arb_domain_mutation() -> BoxedStrategy<Mutation> {
    prop_oneof![
        arb_lifecycle_mutation(),
        arb_structure_mutation(),
        arb_state_mutation(),
        arb_lineage_and_entity_mutation()
    ]
    .boxed()
}

/// A conflict resolution.
pub fn arb_conflict_resolution() -> BoxedStrategy<ConflictResolution> {
    prop_oneof![
        Just(ConflictResolution::KeepJourney),
        Just(ConflictResolution::TakeRoute),
        prop::collection::btree_map(arb_slug(), arb_slug(), 0..2)
            .prop_map(|map| ConflictResolution::MapChoices { map }),
        Just(ConflictResolution::ClearState),
        Just(ConflictResolution::Reopen),
        arb_role_key().prop_map(|role| ConflictResolution::RemapRole { role }),
        arb_kind_key().prop_map(|kind| ConflictResolution::RemapKind { kind }),
        Just(ConflictResolution::Remove),
    ]
    .boxed()
}

fn arb_source() -> BoxedStrategy<ParticipationSource<KeyRefs>> {
    prop_oneof![
        arb_role_key().prop_map(ParticipationSource::Role),
        arb_entity_set().prop_map(ParticipationSource::Entities),
    ]
    .boxed()
}

fn arb_role_reference() -> BoxedStrategy<RoleReference> {
    prop_oneof![
        (arb_node_key(), arb_kind_key())
            .prop_map(|(node, kind)| RoleReference::Participation { node, kind }),
        (arb_node_key(), arb_kind_key())
            .prop_map(|(node, kind)| RoleReference::SegmentParticipation { node, kind }),
        arb_node_key().prop_map(|node| RoleReference::FillsRole { node }),
        arb_entity_set().prop_map(|entities| RoleReference::Fill { entities }),
        Just(RoleReference::DefaultOwner),
        (arb_node_key(), arb_resource::<KeyRefs>())
            .prop_map(|(node, resource)| RoleReference::Draft { node, resource }),
    ]
    .boxed()
}

/// A conflict, of every kind.
pub fn arb_conflict() -> BoxedStrategy<Conflict> {
    let value = || arb_node_field_value::<KeyRefs>();
    prop_oneof![
        (arb_node_key(), value(), value(), any::<bool>()).prop_map(
            |(node, journey, route, dangling)| Conflict::Field {
                dangling,
                node,
                journey,
                route
            }
        ),
        (arb_edge(), any::<bool>()).prop_map(|(edge, journey)| Conflict::Edge {
            edge,
            journey,
            route: !journey
        }),
        (
            arb_node_key(),
            arb_kind_key(),
            prop::option::of(arb_source()),
            prop::option::of(arb_source())
        )
            .prop_map(|(node, kind, journey, route)| Conflict::Participation {
                node,
                kind,
                journey,
                route
            }),
        (
            arb_node_key(),
            arb_attachment_key(),
            prop::option::of(arb_resource::<KeyRefs>()),
            prop::option::of(arb_resource::<KeyRefs>())
        )
            .prop_map(|(node, resource, journey, route)| Conflict::Resource {
                dangling: false,
                node,
                resource,
                journey,
                route
            }),
        (arb_node::<KeyRefs>(), arb_node::<KeyRefs>(), any::<bool>()).prop_map(
            |(journey, route, answered)| Conflict::Shape {
                dangling: false,
                journey: Box::new(journey),
                route: Box::new(route),
                answered
            }
        ),
        (
            arb_node_key(),
            arb_answer_value(),
            prop::option::of(arb_markdown()),
            arb_choices()
        )
            .prop_map(|(decision, answer, rationale, choices)| Conflict::Answer {
                decision,
                answer,
                rationale,
                choices
            }),
        arb_graph_conflict(),
    ]
    .boxed()
}

/// A conflict over a role, a kind, or the default owner.
fn arb_graph_conflict() -> BoxedStrategy<Conflict> {
    prop_oneof![
        (
            arb_role_key(),
            prop::option::of(arb_role::<KeyRefs>()),
            prop::option::of(arb_role::<KeyRefs>()),
            prop::collection::btree_set(arb_role_reference(), 0..3),
            prop::collection::btree_set(arb_insertion_key(), 0..2),
            arb_entity_set()
        )
            .prop_map(|(role, journey, route, references, insertions, members)| {
                Conflict::Role {
                    role,
                    journey,
                    route,
                    references,
                    insertions,
                    members,
                }
            }),
        (
            arb_kind_key(),
            prop::option::of(arb_participation_kind::<KeyRefs>()),
            prop::option::of(arb_participation_kind::<KeyRefs>()),
            prop::collection::btree_map(arb_node_key(), arb_source(), 0..2),
            prop::collection::btree_set(arb_insertion_key(), 0..2)
        )
            .prop_map(
                |(kind, journey, route, references, insertions)| Conflict::Kind {
                    kind,
                    journey,
                    route,
                    references,
                    insertions
                }
            ),
        (
            prop::option::of(arb_role_key()),
            prop::option::of(arb_role_key())
        )
            .prop_map(|(journey, route)| Conflict::DefaultOwner { journey, route }),
    ]
    .boxed()
}

/// A review item.
pub fn arb_review_item() -> BoxedStrategy<ReviewItem> {
    let mapping = prop_oneof![
        Just(ParticipationMapping::Drop),
        arb_role_key().prop_map(ParticipationMapping::Role),
        arb_role::<KeyRefs>().prop_map(ParticipationMapping::NewRole),
        Just(ParticipationMapping::DefaultOwner),
    ];
    let kept = prop_oneof![
        (arb_node_key(), arb_local_edit()).prop_map(|(node, edit)| Kept::Node { node, edit }),
        arb_role_key().prop_map(Kept::Role),
        arb_kind_key().prop_map(Kept::Kind),
        Just(Kept::DefaultOwner),
    ];
    let uses = prop::collection::btree_set(
        (arb_node_key(), arb_kind_key()).prop_map(|(node, kind)| ParticipationRef { node, kind }),
        1..3,
    );
    prop_oneof![
        (arb_conflict(), prop::option::of(arb_conflict_resolution())).prop_map(
            |(conflict, resolution)| ReviewItem::Conflict {
                conflict,
                resolution
            }
        ),
        kept.prop_map(|kept| ReviewItem::KeptLocalEdit { kept }),
        (arb_node_key(), any::<bool>(), arb_removal()).prop_map(|(node, keep, removal)| {
            ReviewItem::Orphan {
                node,
                keep,
                removal,
            }
        }),
        (arb_entity_key(), uses, prop::option::of(mapping)).prop_map(|(entity, uses, mapping)| {
            ReviewItem::Participation {
                entity,
                uses,
                mapping,
            }
        }),
        (arb_node_key(), any::<bool>())
            .prop_map(|(node, excluded)| ReviewItem::Exclusion { node, excluded }),
        arb_removal().prop_map(|removal| ReviewItem::Cascade { removal }),
        // A violation's chains carry mutations, which carry proposals, so the item's
        // violation is built here, chainless, to keep the strategy from recursing.
        (arb_node_key(), arb_title()).prop_map(|(node, message)| ReviewItem::Violation {
            violation: crate::rejection::Violation {
                code: crate::rejection::ViolationCode::DependencyCycle,
                at: crate::rejection::Location {
                    subject: Some(Subject::Node(node)),
                    ..crate::rejection::Location::default()
                },
                message: message.to_string(),
                related: Vec::new(),
                limit: None,
                bypassable: None,
                failures: std::collections::BTreeSet::new(),
                chains: None,
                caused_by: std::collections::BTreeSet::new(),
            }
        }),
    ]
    .boxed()
}

/// A proposal's content.
pub fn arb_proposal_draft() -> BoxedStrategy<ProposalDraft> {
    (
        arb_title(),
        prop::option::of(arb_markdown()),
        arb_revision(),
        prop::collection::vec(arb_domain_mutation(), 0..3),
        prop::collection::vec(arb_review_item(), 0..2),
    )
        .prop_map(
            |(title, description, destination_revision, mutations, items)| ProposalDraft {
                title,
                description,
                destination_revision,
                mutations: bounded_vec(mutations),
                items: bounded_vec(items),
            },
        )
        .boxed()
}

/// A stored proposal.
pub fn arb_proposal() -> BoxedStrategy<Proposal> {
    let status = prop::sample::select(vec![
        ProposalStatus::Open,
        ProposalStatus::Applied,
        ProposalStatus::Discarded,
    ]);
    (
        arb_proposal_id(),
        arb_domain(),
        arb_revision(),
        status,
        arb_proposal_draft(),
        prop::option::of(arb_agent_id()),
        arb_user_id(),
        arb_timestamp(),
    )
        .prop_map(
            |(
                id,
                destination,
                revision,
                status,
                draft,
                proposing_agent,
                created_by,
                created_at,
            )| Proposal {
                id,
                destination,
                revision,
                status,
                draft,
                proposing_agent,
                created_by,
                created_at,
            },
        )
        .boxed()
}

/// Any mutation, proposal edits included.
pub fn arb_mutation() -> BoxedStrategy<Mutation> {
    prop_oneof![
        4 => arb_domain_mutation(),
        1 => prop_oneof![
            arb_proposal_draft().prop_map(|proposal| Mutation::CreateProposal { proposal }),
            arb_proposal_draft().prop_map(|proposal| Mutation::EditProposal { proposal }),
            (arb_proposal_id(), arb_revision()).prop_map(|(proposal, reviewed_revision)| Mutation::ApplyProposal { proposal, reviewed_revision }),
            Just(Mutation::DiscardProposal {}),
        ],
    ]
    .boxed()
}

/// A patch target.
pub fn arb_patch_target() -> BoxedStrategy<PatchTarget> {
    prop_oneof![
        arb_journey_id().prop_map(PatchTarget::Journey),
        arb_route_id().prop_map(PatchTarget::Route),
        Just(PatchTarget::Deployment),
        (arb_proposal_id(), arb_domain())
            .prop_map(|(id, destination)| PatchTarget::Proposal { id, destination }),
    ]
    .boxed()
}

/// A patch.
pub fn arb_patch() -> BoxedStrategy<Patch> {
    (
        arb_patch_id(),
        arb_patch_target(),
        arb_revision(),
        prop::option::of(arb_revision()),
        prop::collection::vec(arb_mutation(), 1..5),
    )
        .prop_map(
            |(id, target, base_revision, deployment_revision, mutations)| Patch {
                id,
                target,
                base_revision,
                deployment_revision,
                mutations: match Mutations::new(mutations) {
                    Ok(mutations) => mutations,
                    Err(error) => panic!("strategy produced bad mutations: {error}"),
                },
            },
        )
        .boxed()
}

fn arb_retired_key() -> BoxedStrategy<RetiredKey> {
    prop_oneof![
        arb_node_key().prop_map(RetiredKey::Node),
        arb_role_key().prop_map(RetiredKey::Role),
        arb_kind_key().prop_map(RetiredKey::Kind)
    ]
    .boxed()
}

fn arb_content_record() -> BoxedStrategy<GraphRecord> {
    let source = prop_oneof![
        arb_role_key().prop_map(ParticipationSource::Role),
        arb_entity_set().prop_map(ParticipationSource::Entities),
    ];
    prop_oneof![
        arb_node::<KeyRefs>().prop_map(GraphRecord::Node),
        (arb_node_key(), arb_node_field_value::<KeyRefs>())
            .prop_map(|(node, value)| GraphRecord::NodeField { node, value }),
        arb_edge().prop_map(GraphRecord::Edge),
        arb_role::<KeyRefs>().prop_map(GraphRecord::Role),
        arb_participation_kind::<KeyRefs>().prop_map(GraphRecord::Kind),
        arb_role_key().prop_map(GraphRecord::DefaultOwner),
        (arb_node_key(), arb_kind_key(), source)
            .prop_map(|(node, kind, source)| GraphRecord::Participation { node, kind, source }),
        (arb_node_key(), arb_resource::<KeyRefs>())
            .prop_map(|(node, resource)| GraphRecord::Resource { node, resource }),
        arb_retired_key().prop_map(GraphRecord::RetiredKey),
        arb_insertion().prop_map(GraphRecord::Insertion),
    ]
    .boxed()
}

fn arb_state_record() -> BoxedStrategy<GraphRecord> {
    prop_oneof![
        (arb_node_key(), arb_node_state())
            .prop_map(|(node, state)| GraphRecord::NodeState { node, state }),
        (arb_node_key(), arb_local_edit())
            .prop_map(|(node, edit)| GraphRecord::LocalEdit { node, edit }),
        (
            arb_node_key(),
            arb_answer_value(),
            prop::option::of(arb_markdown())
        )
            .prop_map(|(decision, value, rationale)| GraphRecord::Answer {
                decision,
                value,
                rationale
            }),
        (arb_role_key(), arb_entity_set())
            .prop_map(|(role, entities)| GraphRecord::RoleFill { role, entities }),
        (arb_node_key(), arb_date()).prop_map(|(node, date)| GraphRecord::Pin { node, date }),
        (arb_node_key(), arb_snooze_target())
            .prop_map(|(node, until)| GraphRecord::Snooze { node, until }),
        (arb_node_key(), arb_overrides())
            .prop_map(|(node, overrides)| GraphRecord::Overrides { node, overrides }),
        arb_node_key().prop_map(GraphRecord::Tombstone),
        arb_annotation().prop_map(GraphRecord::Annotation),
    ]
    .boxed()
}

fn arb_domain_record() -> BoxedStrategy<Record> {
    prop_oneof![
        arb_journey_header().prop_map(Record::JourneyHeader),
        arb_route_header().prop_map(Record::RouteHeader),
        (arb_route_id(), prop::option::of(arb_version_number()))
            .prop_map(|(route, extends)| Record::RouteDraft { route, extends }),
        (arb_route_id(), arb_version_number(), arb_timestamp()).prop_map(
            |(route, version, published_at)| Record::RouteVersion {
                route,
                version,
                published_at
            }
        ),
        (arb_journey_id(), arb_timestamp()).prop_map(|(journey, deleted_at)| {
            Record::DeletedJourney {
                journey,
                deleted_at,
            }
        }),
        arb_entity().prop_map(Record::Entity),
        (arb_entity_key(), arb_entity_key())
            .prop_map(|(alias, entity)| Record::EntityAlias { alias, entity }),
        arb_proposal().prop_map(Record::Proposal),
    ]
    .boxed()
}

/// A stored record.
pub fn arb_record() -> BoxedStrategy<Record> {
    let in_graph = (
        arb_graph_id(),
        prop_oneof![arb_content_record(), arb_state_record()],
    )
        .prop_map(|(graph, record)| Record::Graph { graph, record });
    prop_oneof![2 => in_graph, 1 => arb_domain_record()].boxed()
}

/// A write.
pub fn arb_write() -> BoxedStrategy<Write> {
    prop_oneof![
        4 => arb_record().prop_map(Write::Put),
        2 => arb_record().prop_map(|record| Write::Remove(record.key())),
        1 => arb_graph_id().prop_map(|graph| Write::Remove(RecordKey::Graph(graph))),
        1 => (arb_graph_id(), arb_graph_id()).prop_map(|(from, to)| Write::CopyGraph { from, to }),
    ]
    .boxed()
}

/// An event for one mutation of `patch`, with a generated delta.
pub fn arb_event_for(patch: Patch, ordinal: usize) -> BoxedStrategy<Event> {
    let mutation_index = ordinal % patch.mutations.len();
    (
        arb_user_id(),
        prop::option::of(arb_agent_id()),
        prop::option::of(arb_user_id()),
        arb_timestamp(),
        prop::option::of(arb_markdown()),
        prop::collection::vec(arb_write(), 0..3),
    )
        .prop_map(move |(user, agent, confirming_user, at, note, delta)| {
            let mutation = &patch.mutations.as_slice()[mutation_index];
            Event {
                patch_id: patch.id.clone(),
                ordinal: u32::try_from(mutation_index).unwrap_or(u32::MAX),
                log: patch.target.domain(),
                event_type: mutation.event_type(),
                actor: Actor { user, agent },
                confirming_user,
                subject: mutation.subject(&patch.target),
                at,
                note,
                delta,
            }
        })
        .boxed()
}

/// A change set for a generated patch: one event per mutation.
pub fn arb_change_set() -> BoxedStrategy<ChangeSet> {
    arb_patch()
        .prop_flat_map(|patch| {
            let events: Vec<_> = (0..patch.mutations.len())
                .map(|ordinal| arb_event_for(patch.clone(), ordinal))
                .collect();
            let receipt = PatchReceipt {
                patch_id: patch.id.clone(),
                domain: patch.target.domain(),
                content_hash: patch.content_hash(),
                revision: patch.base_revision.next(),
            };
            events.prop_map(move |events| ChangeSet {
                receipt: receipt.clone(),
                events,
            })
        })
        .boxed()
}

/// A subject.
pub fn arb_subject() -> BoxedStrategy<Subject> {
    prop_oneof![
        arb_node_key().prop_map(Subject::Node),
        arb_entity_key().prop_map(Subject::Entity),
        Just(Subject::Deployment)
    ]
    .boxed()
}

/// A journey scenario of one to three steps.
pub fn arb_scenario() -> BoxedStrategy<crate::scenario::Scenario> {
    let step = (
        arb_markdown(),
        arb_date(),
        arb_timestamp(),
        arb_user_id(),
        prop::option::of(arb_agent_id()),
        arb_patch(),
    )
        .prop_map(
            |(note, today, at, user, agent, patch)| crate::scenario::ScenarioStep {
                note,
                today,
                at,
                actor: Actor { user, agent },
                patch,
            },
        );
    (
        arb_journey_id(),
        arb_markdown(),
        prop::collection::vec(step, 1..3),
    )
        .prop_map(|(journey, description, steps)| crate::scenario::Scenario {
            journey,
            description,
            steps: bounded_vec(steps),
        })
        .boxed()
}
