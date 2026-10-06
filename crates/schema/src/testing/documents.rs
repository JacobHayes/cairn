//! Strategies for the documents: route files, graphs with journey state, and domains.

use proptest::prelude::*;

use super::model::{bounded_set, bounded_vec, keyed};

use super::{
    ArbRefs, arb_annotation, arb_attachment_key, arb_date, arb_email, arb_entity_key,
    arb_entity_set, arb_journey_id, arb_kind_key, arb_markdown, arb_node, arb_node_field,
    arb_node_key, arb_reason, arb_revision, arb_role_key, arb_route_id, arb_slug, arb_timestamp,
    arb_title, arb_version_number,
};

use crate::domain::{
    Deployment, Entity, Journey, JourneyHeader, JourneyStatus, Lineage, Route, RouteDraft,
    RouteHeader, RouteVersion,
};
use crate::graph::{FormatVersion, Graph, ParticipationKind, RetiredKeys, Role, RouteFile};
use crate::refs::{FileRefs, KeyRefs};
use crate::state::{
    AnswerText, AnswerValue, Bypass, Guard, GuardFailure, JourneyState, LocalEdit, NodeState,
    Overrides, Provenance, SnoozeTarget, State,
};

/// A role.
pub fn arb_role<R: ArbRefs>() -> impl Strategy<Value = Role<R>> {
    (
        R::arb_role_key_slot(),
        arb_slug(),
        prop::option::of(arb_title()),
        any::<bool>(),
    )
        .prop_map(|(key, id, title, multi)| Role {
            key,
            id,
            title,
            multi,
        })
}

/// A participation kind.
pub fn arb_participation_kind<R: ArbRefs>() -> impl Strategy<Value = ParticipationKind<R>> {
    (
        R::arb_kind_key_slot(),
        arb_slug(),
        prop::option::of(arb_title()),
        any::<bool>(),
    )
        .prop_map(|(key, id, title, multi)| ParticipationKind {
            key,
            id,
            title,
            multi,
        })
}

/// A route file (the file document).
pub fn arb_route_file() -> impl Strategy<Value = RouteFile> {
    (
        (
            arb_route_id(),
            arb_title(),
            prop::option::of(arb_markdown()),
            prop::option::of(arb_version_number()),
            prop::option::of(arb_slug()),
        ),
        prop::collection::vec(arb_role::<FileRefs>(), 0..3),
        prop::collection::vec(arb_participation_kind::<FileRefs>(), 0..3),
        prop::collection::vec(arb_node::<FileRefs>(), 0..6),
    )
        .prop_map(
            |((route, name, description, extends, default_owner), roles, kinds, nodes)| RouteFile {
                format: FormatVersion,
                route,
                name,
                description,
                extends,
                default_owner,
                roles: bounded_vec(roles),
                participation_kinds: bounded_vec(kinds),
                nodes: bounded_vec(nodes),
            },
        )
}

fn arb_state() -> impl Strategy<Value = State> {
    prop_oneof![
        Just(State::Todo),
        Just(State::Active),
        Just(State::Done),
        Just(State::Skipped),
        Just(State::Open),
        Just(State::Decided),
        Just(State::Pending),
        Just(State::Reached),
        Just(State::Derived),
    ]
}

/// A local-edit marker.
pub fn arb_local_edit() -> impl Strategy<Value = LocalEdit> {
    prop_oneof![
        arb_node_field().prop_map(LocalEdit::Field),
        arb_node_key().prop_map(LocalEdit::Requires),
        arb_kind_key().prop_map(LocalEdit::Participation),
        arb_attachment_key().prop_map(LocalEdit::Resource),
    ]
}

/// A node's stored state.
pub fn arb_node_state() -> impl Strategy<Value = NodeState> {
    (
        arb_state(),
        prop::sample::select(vec![
            Provenance::FromRoute,
            Provenance::Local,
            Provenance::Orphaned,
        ]),
        any::<bool>(),
        prop::option::of(arb_date()),
        prop::option::of(arb_date()),
    )
        .prop_map(
            |(state, provenance, atomic, started_on, finished_on)| NodeState {
                state,
                provenance,
                atomic,
                started_on,
                finished_on,
            },
        )
}

/// An answer of any type.
pub fn arb_answer_value() -> impl Strategy<Value = AnswerValue> {
    prop_oneof![
        any::<bool>().prop_map(AnswerValue::Boolean),
        arb_slug().prop_map(AnswerValue::SingleChoice),
        prop::collection::vec(arb_slug(), 0..3)
            .prop_map(|ids| AnswerValue::MultiChoice(bounded_set(ids))),
        "[A-Za-z ]{0,12}".prop_map(|text| AnswerValue::Text(match AnswerText::try_from(text) {
            Ok(text) => text,
            Err(error) => panic!("strategy exceeded a limit: {error}"),
        })),
        arb_date().prop_map(AnswerValue::Date),
        arb_entity_key().prop_map(AnswerValue::Entity),
        arb_entity_set().prop_map(AnswerValue::EntityList),
    ]
}

/// A guard failure.
pub fn arb_guard_failure() -> impl Strategy<Value = GuardFailure> {
    prop_oneof![
        arb_node_key().prop_map(GuardFailure::OpenDependency),
        Just(GuardFailure::MissingArtifact),
        Just(GuardFailure::NotBrokenDown),
    ]
}

/// A guard.
pub fn arb_guard() -> impl Strategy<Value = Guard> {
    prop::sample::select(vec![Guard::DepsDone, Guard::HasArtifact, Guard::BrokenDown])
}

/// A node's overrides.
pub fn arb_overrides() -> impl Strategy<Value = Overrides> {
    let bypass = (
        prop::collection::btree_set(arb_guard(), 1..3),
        arb_reason(),
        prop::collection::btree_set(arb_guard_failure(), 0..3),
    )
        .prop_map(|(guards, reason, failures)| Bypass {
            guards,
            reason,
            failures,
        });
    (
        prop::option::of(arb_reason()),
        prop::option::of(arb_reason()),
        prop::option::of(bypass),
    )
        .prop_map(|(force_include, keep, bypass)| Overrides {
            force_include,
            keep,
            bypass,
        })
}

/// A snooze target.
pub fn arb_snooze_target() -> impl Strategy<Value = SnoozeTarget> {
    prop_oneof![
        arb_date().prop_map(SnoozeTarget::Date),
        arb_node_key().prop_map(SnoozeTarget::Node)
    ]
}

/// Journey state.
pub fn arb_journey_state() -> impl Strategy<Value = JourneyState> {
    (
        (
            prop::collection::btree_map(arb_node_key(), arb_node_state(), 0..3),
            prop::collection::btree_map(
                arb_node_key(),
                prop::collection::btree_set(arb_local_edit(), 1..3),
                0..2,
            ),
        ),
        prop::collection::btree_map(arb_node_key(), arb_answer_value(), 0..3),
        prop::collection::btree_map(arb_role_key(), arb_entity_set(), 0..2),
        prop::collection::btree_map(arb_node_key(), arb_date(), 0..2),
        prop::collection::btree_map(arb_node_key(), arb_snooze_target(), 0..2),
        prop::collection::btree_map(arb_node_key(), arb_overrides(), 0..2),
        prop::collection::btree_set(arb_node_key(), 0..2),
        prop::collection::vec(arb_annotation(), 0..2),
    )
        .prop_map(
            |(
                (nodes, local_edits),
                answers,
                role_fills,
                pins,
                snoozes,
                overrides,
                tombstones,
                annotations,
            )| {
                JourneyState {
                    nodes,
                    local_edits,
                    answers,
                    role_fills,
                    pins,
                    snoozes,
                    overrides,
                    tombstones,
                    annotations: keyed(annotations),
                }
            },
        )
}

/// A graph document, with or without journey state.
pub fn arb_graph() -> impl Strategy<Value = Graph> {
    (
        prop::option::of(arb_role_key()),
        prop::collection::vec(arb_role::<KeyRefs>(), 0..3),
        prop::collection::vec(arb_participation_kind::<KeyRefs>(), 0..2),
        prop::collection::vec(arb_node::<KeyRefs>(), 0..6),
        (
            prop::collection::btree_set(arb_node_key(), 0..2),
            prop::collection::btree_set(arb_role_key(), 0..2),
            prop::collection::btree_set(arb_kind_key(), 0..2),
        ),
        arb_journey_state(),
    )
        .prop_map(
            |(
                default_owner,
                roles,
                kinds,
                nodes,
                (retired_nodes, retired_roles, retired_kinds),
                state,
            )| Graph {
                default_owner,
                roles: keyed(roles),
                participation_kinds: keyed(kinds),
                nodes: keyed(nodes),
                retired_keys: RetiredKeys {
                    nodes: retired_nodes,
                    roles: retired_roles,
                    kinds: retired_kinds,
                },
                state,
            },
        )
}

/// A lineage.
pub fn arb_lineage() -> impl Strategy<Value = Lineage> {
    (arb_route_id(), arb_version_number()).prop_map(|(route, version)| Lineage { route, version })
}

/// A journey status.
pub fn arb_journey_status() -> impl Strategy<Value = JourneyStatus> {
    prop::sample::select(vec![
        JourneyStatus::Active,
        JourneyStatus::Completed,
        JourneyStatus::Archived,
    ])
}

/// A journey's own fields.
pub fn arb_journey_header() -> impl Strategy<Value = JourneyHeader> {
    (
        arb_journey_id(),
        arb_title(),
        prop::option::of(arb_markdown()),
        arb_journey_status(),
        prop::option::of(arb_lineage()),
        arb_timestamp(),
        arb_date(),
    )
        .prop_map(
            |(id, name, description, status, lineage, created_at, created_on)| JourneyHeader {
                id,
                name,
                description,
                status,
                lineage,
                created_at,
                created_on,
            },
        )
}

/// A journey domain.
pub fn arb_journey() -> impl Strategy<Value = Journey> {
    (arb_journey_header(), arb_revision(), arb_graph()).prop_map(|(header, revision, graph)| {
        Journey {
            header,
            revision,
            graph,
        }
    })
}

/// A route's own fields.
pub fn arb_route_header() -> impl Strategy<Value = RouteHeader> {
    (
        arb_route_id(),
        arb_title(),
        prop::option::of(arb_markdown()),
        any::<bool>(),
    )
        .prop_map(|(id, name, description, retired)| RouteHeader {
            id,
            name,
            description,
            retired,
        })
}

/// A route domain.
pub fn arb_route() -> impl Strategy<Value = Route> {
    let draft = (prop::option::of(arb_version_number()), arb_graph())
        .prop_map(|(extends, graph)| RouteDraft { extends, graph });
    (
        arb_route_header(),
        arb_revision(),
        prop::collection::btree_set(arb_version_number(), 0..3),
        prop::option::of(draft),
    )
        .prop_map(|(header, revision, versions, draft)| Route {
            header,
            revision,
            versions,
            draft,
        })
}

/// A published route version.
pub fn arb_route_version() -> impl Strategy<Value = RouteVersion> {
    (
        arb_route_id(),
        arb_version_number(),
        arb_timestamp(),
        arb_graph(),
    )
        .prop_map(|(route, version, published_at, graph)| RouteVersion {
            route,
            version,
            published_at,
            graph,
        })
}

/// An entity.
pub fn arb_entity() -> impl Strategy<Value = Entity> {
    (
        arb_entity_key(),
        arb_title(),
        prop::collection::btree_set(arb_email(), 0..2),
    )
        .prop_map(|(key, name, emails)| Entity { key, name, emails })
}

/// The deployment domain.
pub fn arb_deployment() -> impl Strategy<Value = Deployment> {
    (
        arb_revision(),
        prop::collection::vec(arb_entity(), 0..3),
        prop::collection::btree_map(arb_entity_key(), arb_entity_key(), 0..2),
    )
        .prop_map(|(revision, entities, aliases)| Deployment {
            revision,
            entities: keyed(entities),
            aliases,
        })
}
