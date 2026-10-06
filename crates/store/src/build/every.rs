//! Values that fill every optional field and hold every variant of each persisted type, and
//! their cleared counterparts, for the conformance cases that compare what each backend
//! loads with them (PRACTICES, Shell: conformance and integration).
//!
//! Each value is built field by field with no `..`, so a field added to a persisted type
//! does not compile here until it has a value. Each enum's variants come from the schema's
//! `ALL` list, or from a [`chain`] written as an exhaustive match, so a variant added does
//! not compile until it joins the chain.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    Action, Annotation, AnnotationBody, AnnotationContent, AnswerSpec, AnswerType, AnswerValue,
    BoundedSet, Bypass, Choices, Date, Days, Decision, Deliverable, Domain, Entity, Graph, Group,
    Guard, GuardFailure, JourneyHeader, JourneyState, JourneyStatus, Keyed, Lineage, LocalEdit,
    Milestone, Node, NodeField, NodeKey, NodeKind, NodeState, Overrides, ParticipationKind,
    ParticipationSource, Participations, Payload, Proposal, ProposalDraft, ProposalStatus,
    Provenance, Resource, ResourceContent, RetiredKeys, Role, RouteHeader, SnoozeTarget, State,
    Weight, refs::KeyRefs,
};
use serde_json::json;

use super::{at, id, revision, title, version};
use crate::records::{AuthEvent, MessageAuthor, OAuthStateKind};

/// Whether a value fills every optional field and sets each flag off its default, or
/// clears them: the second is put over the first, so a backend that keeps a stale column
/// fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fill {
    /// Every optional field set, every flag off its default.
    Every,
    /// Every optional field cleared, every flag at its default.
    Cleared,
}

impl Fill {
    fn some<T>(self, value: T) -> Option<T> {
        (self == Fill::Every).then_some(value)
    }

    /// True when filled: a flag whose default is false.
    fn on(self) -> bool {
        self == Fill::Every
    }

    /// Each variant list starts one later when cleared, so an overwrite changes every
    /// record's variant.
    fn rotate<T>(self, mut values: Vec<T>) -> Vec<T> {
        if self == Fill::Cleared && !values.is_empty() {
            values.rotate_left(1);
        }
        values
    }
}

/// Every variant of an enum, from `first` through `next`, which is written as an exhaustive
/// match: a variant added to the enum does not compile until `next` names it.
fn chain<T>(first: T, next: fn(&T) -> Option<T>) -> Vec<T> {
    std::iter::successors(Some(first), next).collect()
}

fn day(number: i8) -> Date {
    Date::new(2026, 11, number).unwrap()
}

fn node_key(index: usize) -> NodeKey {
    id(&format!("n_{index}"))
}

/// Every node provenance.
#[must_use]
pub fn provenances() -> Vec<Provenance> {
    chain(Provenance::FromRoute, |provenance| match provenance {
        Provenance::FromRoute => Some(Provenance::Local),
        Provenance::Local => Some(Provenance::Orphaned),
        Provenance::Orphaned => None,
    })
}

/// Every journey status.
#[must_use]
pub fn journey_statuses() -> Vec<JourneyStatus> {
    chain(JourneyStatus::Active, |status| match status {
        JourneyStatus::Active => Some(JourneyStatus::Completed),
        JourneyStatus::Completed => Some(JourneyStatus::Archived),
        JourneyStatus::Archived => None,
    })
}

/// Every proposal status.
#[must_use]
pub fn proposal_statuses() -> Vec<ProposalStatus> {
    chain(ProposalStatus::Open, |status| match status {
        ProposalStatus::Open => Some(ProposalStatus::Applied),
        ProposalStatus::Applied => Some(ProposalStatus::Discarded),
        ProposalStatus::Discarded => None,
    })
}

/// Every local edit: one per node field, and each other marker.
#[must_use]
pub fn local_edits() -> BTreeSet<LocalEdit> {
    let others = chain(LocalEdit::Shape, |edit| match edit {
        LocalEdit::Shape => Some(LocalEdit::Requires(node_key(1))),
        LocalEdit::Requires(_) => Some(LocalEdit::Participation(id("k_role"))),
        LocalEdit::Participation(_) => Some(LocalEdit::Resource(id("a_0"))),
        LocalEdit::Resource(_) | LocalEdit::Field(_) => None,
    });
    NodeField::ALL
        .into_iter()
        .map(LocalEdit::Field)
        .chain(others)
        .collect()
}

/// An answer of every type.
#[must_use]
pub fn answer_values() -> Vec<AnswerValue> {
    AnswerType::ALL
        .into_iter()
        .map(|answer| match answer {
            AnswerType::Boolean => AnswerValue::Boolean(true),
            AnswerType::SingleChoice => AnswerValue::SingleChoice(id("wide")),
            AnswerType::MultiChoice => {
                AnswerValue::MultiChoice(BoundedSet::new([id("narrow"), id("wide")]).unwrap())
            }
            AnswerType::Text => AnswerValue::Text("Some remarks.".to_owned().try_into().unwrap()),
            AnswerType::Date => AnswerValue::Date(day(20)),
            AnswerType::Entity => AnswerValue::Entity(id("e_a")),
            AnswerType::EntityList => {
                AnswerValue::EntityList(BoundedSet::new([id("e_a"), id("e_b")]).unwrap())
            }
        })
        .collect()
}

/// Every snooze target.
#[must_use]
pub fn snooze_targets() -> Vec<SnoozeTarget> {
    chain(SnoozeTarget::Date(day(10)), |target| match target {
        SnoozeTarget::Date(_) => Some(SnoozeTarget::Node(node_key(2))),
        SnoozeTarget::Node(_) => None,
    })
}

/// Every resource content, the message draft naming every placeholder kind.
#[must_use]
pub fn resource_contents() -> Vec<ResourceContent<KeyRefs>> {
    chain(
        ResourceContent::Tip(id("Start from the last one.")),
        |content| match content {
            ResourceContent::Tip(_) => Some(ResourceContent::Template(id(
                "https://example.org/template",
            ))),
            ResourceContent::Template(_) => {
                Some(ResourceContent::Example(id("https://example.org/example")))
            }
            ResourceContent::Example(_) => {
                Some(ResourceContent::Reference(id("https://example.org/guide")))
            }
            ResourceContent::Reference(_) => Some(ResourceContent::MessageDraft(id(
                "Hello {{roles.r_lead.name}}: {{journey.name}}, {{journey.description}}, \
                 {{journey.created_at}}, {{journey.url}}, {{journey.status}}; {{answers.n_0}}.",
            ))),
            ResourceContent::MessageDraft(_) => None,
        },
    )
}

/// Every annotation content.
#[must_use]
pub fn annotation_contents() -> Vec<AnnotationContent> {
    chain(
        AnnotationContent::Note(id("Discussed.")),
        |content| match content {
            AnnotationContent::Note(_) => Some(AnnotationContent::Artifact(id(
                "https://example.org/output",
            ))),
            AnnotationContent::Artifact(_) => Some(AnnotationContent::Reference(id(
                "https://example.org/reference",
            ))),
            AnnotationContent::Reference(_) => Some(AnnotationContent::Conversation(id(
                "https://example.org/thread",
            ))),
            AnnotationContent::Conversation(_) => None,
        },
    )
}

/// Every guard and every guard failure, in one bypass.
#[must_use]
pub fn bypass() -> Bypass {
    let guards = chain(Guard::DepsDone, |guard| match guard {
        Guard::DepsDone => Some(Guard::HasArtifact),
        Guard::HasArtifact => Some(Guard::BrokenDown),
        Guard::BrokenDown => None,
    });
    let failures = chain(
        GuardFailure::OpenDependency(node_key(1)),
        |failure| match failure {
            GuardFailure::OpenDependency(_) => Some(GuardFailure::MissingArtifact),
            GuardFailure::MissingArtifact => Some(GuardFailure::NotBrokenDown),
            GuardFailure::NotBrokenDown => None,
        },
    );
    Bypass {
        guards: guards.into_iter().collect(),
        reason: id("Waived for now."),
        failures: failures.into_iter().collect(),
    }
}

/// Every OAuth state kind.
#[must_use]
pub fn oauth_state_kinds() -> Vec<OAuthStateKind> {
    chain(OAuthStateKind::AuthorizationCode, |kind| match kind {
        OAuthStateKind::AuthorizationCode => Some(OAuthStateKind::PkceVerifier),
        OAuthStateKind::PkceVerifier => Some(OAuthStateKind::LoginState),
        OAuthStateKind::LoginState => None,
    })
}

/// Every auth event.
#[must_use]
pub fn auth_events() -> Vec<AuthEvent> {
    chain(AuthEvent::UserCreated, |event| match event {
        AuthEvent::UserCreated => Some(AuthEvent::IdentityLinked),
        AuthEvent::IdentityLinked => Some(AuthEvent::IdentityUnlinked),
        AuthEvent::IdentityUnlinked => Some(AuthEvent::TokenMinted),
        AuthEvent::TokenMinted => Some(AuthEvent::TokenRevoked),
        AuthEvent::TokenRevoked => None,
    })
}

/// Every conversation message author.
#[must_use]
pub fn message_authors() -> Vec<MessageAuthor> {
    chain(MessageAuthor::User, |author| match author {
        MessageAuthor::User => Some(MessageAuthor::Assistant),
        MessageAuthor::Assistant => Some(MessageAuthor::Tool),
        MessageAuthor::Tool => None,
    })
}

/// A decision of each answer type, then a node of each other kind.
fn payloads(fill: Fill) -> Vec<Payload<KeyRefs>> {
    let choices: Choices =
        serde_json::from_value(json!(["narrow", {"id": "wide", "title": "Wide"}])).unwrap();
    let mut payloads = Vec::new();
    for kind in NodeKind::ALL {
        match kind {
            NodeKind::Decision => {
                payloads.extend(AnswerType::ALL.into_iter().map(|answer| {
                    let answer = match answer {
                        AnswerType::Boolean => AnswerSpec::Boolean,
                        AnswerType::SingleChoice => AnswerSpec::SingleChoice(choices.clone()),
                        AnswerType::MultiChoice => AnswerSpec::MultiChoice(choices.clone()),
                        AnswerType::Text => AnswerSpec::Text,
                        AnswerType::Date => AnswerSpec::Date {
                            feeds_milestone: fill.some(node_key(1)),
                        },
                        AnswerType::Entity => AnswerSpec::Entity {
                            fills_role: fill.some(id("r_lead")),
                        },
                        AnswerType::EntityList => AnswerSpec::EntityList {
                            fills_role: fill.some(id("r_other")),
                        },
                    };
                    Payload::Decision(Decision {
                        prompt: id("Which way?"),
                        help: fill.some(id("Pick the one that fits.")),
                        answer,
                    })
                }));
            }
            NodeKind::Deliverable => payloads.push(Payload::Deliverable(Deliverable {
                estimate: fill.some(Days::try_from(3).unwrap()),
                placeholder: fill.on(),
                requires_artifact: fill.on(),
            })),
            NodeKind::Action => payloads.push(Payload::Action(Action {
                estimate: fill.some(Days::try_from(2).unwrap()),
                placeholder: fill.on(),
            })),
            NodeKind::Milestone => payloads.push(Payload::Milestone(Milestone {
                is_final: fill.on(),
                auto_reach: fill.on(),
            })),
            NodeKind::Group => payloads.push(Payload::Group(Group {
                opens_at: fill.some(node_key(1)),
                closes_at: fill.some(node_key(2)),
                gates: !fill.on(),
                closes: !fill.on(),
            })),
        }
    }
    fill.rotate(payloads)
}

/// One node of each payload, keyed `n_0` onward.
fn nodes(fill: Fill) -> Vec<Node<KeyRefs>> {
    let participations = BTreeMap::from([
        (id("k_role"), ParticipationSource::Role(id("r_lead"))),
        (
            id("k_entities"),
            ParticipationSource::Entities(BoundedSet::new([id("e_a"), id("e_b")]).unwrap()),
        ),
        (
            id("k_none"),
            ParticipationSource::Entities(BoundedSet::new([]).unwrap()),
        ),
    ]);
    let resources: Vec<Resource<KeyRefs>> = (0..)
        .zip(resource_contents())
        .map(|(index, content)| Resource {
            key: id(&format!("a_{index}")),
            title: (index % 2 == 0).then(|| title("Guidance")),
            content,
        })
        .collect();
    payloads(fill)
        .into_iter()
        .enumerate()
        .map(|(index, payload)| Node {
            key: node_key(index),
            id: id(&format!("node-{index}")),
            parent: fill.some(id("n_parent")),
            title: title(&format!("Node {index}")),
            description: fill.some(id("What the node is for.")),
            weight: fill.some(Weight::try_from(u32::try_from(index).unwrap() + 2).unwrap()),
            requires: BoundedSet::new(fill.some([node_key(1), node_key(2)]).into_iter().flatten())
                .unwrap(),
            relevant_when: fill.some(serde_json::from_value(json!({"answered": "n_0"})).unwrap()),
            due_by: fill.some(
                serde_json::from_value(
                    json!({"before": ["n_1", "journey.created_at"], "offset": 2}),
                )
                .unwrap(),
            ),
            not_before: fill.some(serde_json::from_value(json!({"after": "n_2"})).unwrap()),
            participations: Participations::try_from(if fill.on() {
                participations.clone()
            } else {
                BTreeMap::new()
            })
            .unwrap(),
            resources: if fill.on() {
                resources.clone()
            } else {
                Vec::new()
            },
            payload,
        })
        .collect()
}

/// Every content record a graph holds: roles and kinds with and without titles, the default
/// owner, a node of every payload with every field, and a retired key of each kind.
#[must_use]
pub fn content(fill: Fill) -> Graph {
    let roles = [("r_lead", "lead"), ("r_other", "other")]
        .into_iter()
        .map(|(key, slug)| Role {
            key: id(key),
            id: id(slug),
            title: fill.some(title("A role")),
            multi: fill.on(),
        });
    let kinds = [("k_role", "role"), ("k_entities", "entities")]
        .into_iter()
        .map(|(key, slug)| ParticipationKind {
            key: id(key),
            id: id(slug),
            title: fill.some(title("A kind")),
            multi: fill.on(),
        });
    Graph {
        default_owner: Some(id(if fill.on() { "r_lead" } else { "r_other" })),
        roles: Keyed::new(roles).unwrap(),
        participation_kinds: Keyed::new(kinds).unwrap(),
        nodes: Keyed::new(nodes(fill)).unwrap(),
        retired_keys: RetiredKeys {
            nodes: BTreeSet::from([id("n_old")]),
            roles: BTreeSet::from([id("r_old")]),
            kinds: BTreeSet::from([id("k_old")]),
        },
        state: JourneyState::default(),
    }
}

/// Every state record a journey holds: a node state in every state and provenance, every
/// local edit, an answer of every type, filled and empty role fills, a pin, every snooze
/// target, overrides with every reason and a bypass of every guard, a tombstone, and an
/// annotation of every content.
#[must_use]
pub fn state(fill: Fill) -> JourneyState {
    let nodes = (0..)
        .zip(fill.rotate(State::ALL.to_vec()))
        .zip(fill.rotate(provenances()).into_iter().cycle())
        .map(|((index, state), provenance)| {
            (
                node_key(index),
                NodeState {
                    state,
                    provenance,
                    atomic: fill.on(),
                    started_on: fill.some(day(1)),
                    finished_on: fill.some(day(2)),
                    skip_reason: fill.some(id("Not needed here.")),
                },
            )
        });
    let annotations = (0..)
        .zip(fill.rotate(annotation_contents()))
        .map(|(index, content)| Annotation {
            body: AnnotationBody {
                key: id(&format!("a_note{index}")),
                node: fill.some(node_key(index)),
                title: fill.some(title("A note")),
                content,
            },
            created_by: id("u_tester"),
            created_at: at(1),
            edited_at: fill.some(at(2)),
        });
    let fills = [("r_lead", vec!["e_a", "e_b"]), ("r_other", Vec::new())];
    JourneyState {
        nodes: nodes.collect(),
        local_edits: BTreeMap::from([(node_key(0), local_edits())]),
        answers: by_node(fill.rotate(answer_values())),
        role_fills: fill
            .rotate(fills.to_vec())
            .into_iter()
            .zip(["r_lead", "r_other"])
            .map(|((_, entities), role)| {
                let entities = entities.into_iter().map(id);
                (id(role), BoundedSet::new(entities).unwrap())
            })
            .collect(),
        pins: BTreeMap::from([(node_key(0), day(if fill.on() { 3 } else { 4 }))]),
        snoozes: by_node(fill.rotate(snooze_targets())),
        overrides: BTreeMap::from([(
            node_key(0),
            Overrides {
                force_include: fill.some(id("Needed either way.")),
                keep: fill.some(id("Keep it under a skip.")),
                bypass: fill.some(bypass()),
            },
        )]),
        tombstones: BTreeSet::from([id("n_gone")]),
        annotations: Keyed::new(annotations).unwrap(),
    }
}

/// Each value on its own node, `n_0` onward.
fn by_node<T>(values: Vec<T>) -> BTreeMap<NodeKey, T> {
    (0..).map(node_key).zip(values).collect()
}

/// [`content`] with [`state`].
#[must_use]
pub fn graph(fill: Fill) -> Graph {
    let Graph {
        default_owner,
        roles,
        participation_kinds,
        nodes,
        retired_keys,
        state: _,
    } = content(fill);
    Graph {
        default_owner,
        roles,
        participation_kinds,
        nodes,
        retired_keys,
        state: state(fill),
    }
}

/// A journey's fields in `status`, following a route version when filled.
#[must_use]
pub fn journey_header(journey: &str, status: JourneyStatus, fill: Fill) -> JourneyHeader {
    JourneyHeader {
        id: id(journey),
        name: title(if fill.on() { "Every field" } else { "Cleared" }),
        description: fill.some(id("A journey with every field.")),
        status,
        lineage: fill.some(Lineage {
            route: id("every"),
            version: version(1),
        }),
        created_at: at(0),
        created_on: day(1),
    }
}

/// A route's fields, retired when filled.
#[must_use]
pub fn route_header(route: &str, fill: Fill) -> RouteHeader {
    RouteHeader {
        id: id(route),
        name: title(if fill.on() { "Every field" } else { "Cleared" }),
        description: fill.some(id("A route with every field.")),
        retired: fill.on(),
    }
}

/// An entity, with emails when filled.
#[must_use]
pub fn entity(key: &str, fill: Fill) -> Entity {
    let emails = fill.some(["e@example.org", "f@example.org"]);
    Entity {
        key: id(key),
        name: title(if fill.on() { "Every field" } else { "Cleared" }),
        emails: emails.into_iter().flatten().map(id).collect(),
    }
}

/// A list from its written form when filled, or empty.
fn list<T: serde::de::DeserializeOwned>(fill: Fill, filled: serde_json::Value) -> T {
    serde_json::from_value(if fill.on() { filled } else { json!([]) }).unwrap()
}

/// A proposal in `status` against `destination` at `at_revision`, with a mutation and a
/// review item when filled.
#[must_use]
pub fn proposal(
    key: &str,
    destination: Domain,
    at_revision: u32,
    status: ProposalStatus,
    fill: Fill,
) -> Proposal {
    let mutations = json!([{"op": "add_node", "node": {
        "key": "n_review", "id": "review", "kind": "action", "title": "Review",
    }}]);
    let items =
        json!([{"item": "orphan", "node": "n_gone", "keep": true, "removal": {"node": "n_gone"}}]);
    Proposal {
        id: id(key),
        destination,
        revision: revision(at_revision),
        status,
        draft: ProposalDraft {
            title: title("Add a review step"),
            description: fill.some(id("A step to review the plan.")),
            destination_revision: revision(1),
            mutations: list(fill, mutations),
            items: list(fill, items),
        },
        proposing_agent: fill.some(id("ag_assistant")),
        created_by: id("u_tester"),
        created_at: at(0),
    }
}
