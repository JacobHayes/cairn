//! Strategies for the documents: route files, graphs with journey state, and domains.

use proptest::prelude::*;

use super::model::{bounded_set, bounded_vec, keyed};

use super::{
    ArbRefs, arb_annotation, arb_date, arb_email, arb_identity_key, arb_identity_set,
    arb_journey_id, arb_kind_key, arb_markdown, arb_node, arb_node_key, arb_reason, arb_revision,
    arb_role_key, arb_route_id, arb_slug, arb_timestamp, arb_title, arb_version_number, arb_when,
};

use crate::domain::{
    Deployment, Identity, Journey, JourneyHeader, JourneyStatus, Lineage, Route, RouteDraft,
    RouteHeader, RouteVersion,
};
use crate::graph::{FormatVersion, Graph, ParticipationKind, RetiredKeys, Role, RouteFile};
use crate::refs::{FileRefs, KeyRefs};
use crate::state::{
    AnswerText, AnswerValue, Bypass, Guard, GuardFailure, JourneyState, NodeState, Overrides,
    Reopened, SnoozeTarget, StaleReason, State,
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
    prop::sample::select(State::ALL.to_vec())
}

/// A node's stored state.
pub fn arb_node_state() -> impl Strategy<Value = NodeState> {
    (
        arb_state(),
        any::<bool>(),
        prop::option::of(arb_date()),
        prop::option::of((arb_date(), arb_revision())),
        prop::option::of(arb_reason()),
        prop::option::of((arb_revision(), arb_revision())),
    )
        .prop_map(
            |(state, atomic, started_on, finished, reason, reopened)| NodeState {
                state,
                atomic,
                started_on,
                finished_on: finished.map(|(day, _)| day),
                finished_revision: finished.map(|(_, revision)| revision),
                reason,
                reopened: reopened.map(|(revision, finished_revision)| Reopened {
                    revision,
                    finished_revision,
                }),
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
        arb_when().prop_map(AnswerValue::Date),
        arb_identity_key().prop_map(AnswerValue::Identity),
        arb_identity_set().prop_map(AnswerValue::IdentityList),
    ]
}

/// A guard failure.
pub fn arb_guard_failure() -> impl Strategy<Value = GuardFailure> {
    prop_oneof![
        Just(GuardFailure::MissingArtifact),
        Just(GuardFailure::MissingNote),
        arb_slug().prop_map(GuardFailure::Unticked),
        Just(GuardFailure::NotBrokenDown),
    ]
}

/// Why a finished node is stale.
pub fn arb_stale_reason() -> impl Strategy<Value = StaleReason> {
    prop_oneof![
        arb_node_key().prop_map(StaleReason::Reopened),
        arb_guard_failure().prop_map(StaleReason::from),
    ]
}

/// A guard.
pub fn arb_guard() -> impl Strategy<Value = Guard> {
    prop::sample::select(vec![
        Guard::HasArtifact,
        Guard::HasNote,
        Guard::AllTicked,
        Guard::BrokenDown,
    ])
}

/// A node's overrides.
pub fn arb_overrides() -> impl Strategy<Value = Overrides> {
    let bypass = (
        prop::option::of(arb_reason()),
        prop::collection::btree_set(arb_guard_failure(), 0..3),
    )
        .prop_map(|(reason, failures)| Bypass { reason, failures });
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
        arb_when().prop_map(SnoozeTarget::Date),
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
                prop::collection::btree_set(arb_slug(), 1..3),
                0..2,
            ),
        ),
        // A rationale is held only beside its answer (B2).
        prop::collection::btree_map(
            arb_node_key(),
            (arb_answer_value(), prop::option::of(arb_markdown())),
            0..3,
        ),
        prop::collection::btree_map(arb_role_key(), arb_identity_set(), 0..2),
        prop::collection::btree_map(arb_node_key(), arb_when(), 0..2),
        prop::collection::btree_map(arb_node_key(), arb_snooze_target(), 0..2),
        prop::collection::btree_map(arb_node_key(), arb_overrides(), 0..2),
        prop::collection::vec(arb_annotation(), 0..2),
    )
        .prop_map(
            |((nodes, ticks), answered, role_fills, pins, snoozes, overrides, annotations)| {
                let mut answers = std::collections::BTreeMap::new();
                let mut rationales = std::collections::BTreeMap::new();
                for (decision, (value, rationale)) in answered {
                    rationales.extend(rationale.map(|text| (decision.clone(), text)));
                    answers.insert(decision, value);
                }
                JourneyState {
                    nodes,
                    ticks,
                    answers,
                    rationales,
                    role_fills,
                    pins,
                    snoozes,
                    overrides,
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
        arb_identity_set(),
        arb_journey_status(),
        prop::option::of(arb_lineage()),
        arb_timestamp(),
        arb_date(),
    )
        .prop_map(
            |(id, name, description, owners, status, lineage, created_at, created_on)| {
                JourneyHeader {
                    id,
                    name,
                    description,
                    owners,
                    status,
                    lineage,
                    created_at,
                    created_on,
                }
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

/// An identity.
pub fn arb_identity() -> impl Strategy<Value = Identity> {
    (
        arb_identity_key(),
        prop::option::of(arb_title()),
        prop::collection::btree_set(arb_email(), 0..2),
        arb_email(),
    )
        .prop_map(|(key, name, mut emails, spare)| {
            // H3: an identity has a name, an email, or both.
            if name.is_none() && emails.is_empty() {
                emails.insert(spare);
            }
            Identity { key, name, emails }
        })
}

/// The deployment domain.
pub fn arb_deployment() -> impl Strategy<Value = Deployment> {
    (
        arb_revision(),
        prop::collection::vec(arb_identity(), 0..3),
        prop::collection::btree_map(arb_identity_key(), arb_identity_key(), 0..2),
    )
        .prop_map(|(revision, identities, aliases)| Deployment {
            revision,
            identities: keyed(identities),
            aliases,
        })
}
