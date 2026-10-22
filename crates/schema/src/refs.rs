//! The two forms of one document schema (ARCHITECTURE, File format): the file form, as
//! written, refers to nodes by path and to roles and kinds by id, and may leave keys out
//! for import to match or mint; the graph form, resolved, refers to everything by key and
//! always carries keys. The graph types are generic over [`References`], so the file
//! document and the graph document are two types with one shape (PRD, Keys and
//! references: references are stored by key; paths and ids are the serialization form).

use std::collections::BTreeMap;
use std::fmt;
use std::hash::Hash;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::attachment::{MessageTemplate, Resource, ResourceContent, Segment, TemplateRefusal};
use crate::field::NodeField;
use crate::id::{AttachmentKey, KindKey, NodeKey, Path, RoleKey, Slug};
use crate::node::{
    AnswerSpec, DateRule, DateSource, Decision, Group, Node, ParticipationSource, Participations,
    Payload,
};

/// A value one document form uses to refer to something: text in both formats.
pub trait Reference:
    Clone
    + fmt::Debug
    + fmt::Display
    + FromStr<Err: fmt::Display>
    + Ord
    + Hash
    + Serialize
    + DeserializeOwned
    + JsonSchema
    + Send
    + Sync
    + 'static
{
}

impl<T> Reference for T where
    T: Clone
        + fmt::Debug
        + fmt::Display
        + FromStr<Err: fmt::Display>
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned
        + JsonSchema
        + Send
        + Sync
        + 'static
{
}

/// Where an object's own key goes: optional in a file (import matches or mints a missing
/// one), required in a graph.
pub trait KeySlot:
    Clone + fmt::Debug + Ord + Hash + Serialize + DeserializeOwned + JsonSchema + Send + Sync + 'static
{
    /// The key, when there is one.
    type Key;

    /// True when the key is left out, so serialization omits the field.
    fn is_absent(&self) -> bool;
}

impl<K> KeySlot for Option<K>
where
    K: Clone
        + fmt::Debug
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned
        + JsonSchema
        + Send
        + Sync
        + 'static,
{
    type Key = K;

    fn is_absent(&self) -> bool {
        self.is_none()
    }
}

macro_rules! required_key_slot {
    ($($key:ty),*) => {$(
        impl KeySlot for $key {
            type Key = $key;

            fn is_absent(&self) -> bool {
                false
            }
        }
    )*};
}

required_key_slot!(NodeKey, RoleKey, KindKey, AttachmentKey);

/// The reference and key types of one document form.
pub trait References:
    Clone + fmt::Debug + PartialEq + Eq + PartialOrd + Ord + Hash + JsonSchema + Send + Sync + 'static
{
    /// A reference to a node: a path in a file, a key in a graph.
    type Node: Reference;
    /// A reference to a role: an id in a file, a key in a graph.
    type Role: Reference;
    /// A reference to a participation kind: an id in a file, a key in a graph.
    type Kind: Reference;
    /// A node's own key.
    type NodeKey: KeySlot<Key = NodeKey>;
    /// A role's own key.
    type RoleKey: KeySlot<Key = RoleKey>;
    /// A participation kind's own key.
    type KindKey: KeySlot<Key = KindKey>;
    /// A resource's own key.
    type AttachmentKey: KeySlot<Key = AttachmentKey>;
    /// The reference to the built-in `owner` kind.
    fn owner_kind() -> Self::Kind;
}

/// The file form (unresolved, as written): paths and ids, optional keys.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileRefs {}

/// The graph form (resolved): keys everywhere.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KeyRefs {}

impl References for FileRefs {
    type Node = Path;
    type Role = Slug;
    type Kind = Slug;
    type NodeKey = Option<NodeKey>;
    type RoleKey = Option<RoleKey>;
    type KindKey = Option<KindKey>;
    type AttachmentKey = Option<AttachmentKey>;
    fn owner_kind() -> Slug {
        crate::id::owner_kind_id()
    }
}

impl References for KeyRefs {
    type Node = NodeKey;
    type Role = RoleKey;
    type Kind = KindKey;
    type NodeKey = NodeKey;
    type RoleKey = RoleKey;
    type KindKey = KindKey;
    type AttachmentKey = AttachmentKey;
    fn owner_kind() -> KindKey {
        KindKey::owner()
    }
}

/// `skip_serializing_if` for a key slot.
pub(crate) fn key_absent<S: KeySlot>(slot: &S) -> bool {
    slot.is_absent()
}

// The form's schema name is the suffix generic types carry in the JSON Schema
// (`#[schemars(rename = "Node{R}")]`): nothing for the file form, which is the one the
// published schema describes, and `Resolved` for the graph form.
impl JsonSchema for FileRefs {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!(false)
    }
}

impl JsonSchema for KeyRefs {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Resolved".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!(false)
    }
}

// ARCHITECTURE: File format. PRD: A13, B13.
/// Translates every reference of a node from one document form to another: an insertion
/// from the segment's keys to the graph's, an export from keys to paths and ids, an import
/// from paths and ids to keys. [`Node::map_references`] is the one walk over a node's
/// fields, generic over both forms, so a new reference field cannot compile without being
/// translated, even when both forms are one.
///
/// A reference the translator answers none for is left out of the result, as import does
/// with one it reports; the other two translate every reference.
pub trait MapReferences<A: References, B: References> {
    /// A reference to a node. `field` is where it sits, for a violation's location: none
    /// for a requirement, or a message draft's placeholder.
    fn node(&mut self, node: &A::Node, field: Option<NodeField>) -> Option<B::Node>;

    /// A reference to a role, located as [`MapReferences::node`] is.
    fn role(&mut self, role: &A::Role, field: Option<NodeField>) -> Option<B::Role>;

    /// A reference to a participation kind.
    fn kind(&mut self, kind: &A::Kind) -> Option<B::Kind>;

    /// The node's own key.
    fn own_key(&mut self, key: &A::NodeKey) -> B::NodeKey;

    /// A resource's own key. Asked once per resource, in order, even for one that is left
    /// out.
    fn resource_key(&mut self, key: &A::AttachmentKey) -> B::AttachmentKey;

    /// A message draft that passes the body limit once translated; the resource is left out.
    fn draft_too_long(&mut self, resource: &A::AttachmentKey, bytes: usize);
}

impl<A: References> Node<A> {
    /// The node with every reference translated by `map`, asked in the order of the node's
    /// fields so what a translator reports comes in that order.
    #[must_use]
    pub fn map_references<B: References>(&self, map: &mut impl MapReferences<A, B>) -> Node<B> {
        let parent = self
            .parent
            .as_ref()
            .and_then(|parent| map.node(parent, Some(NodeField::Parent)));
        let requires: Vec<B::Node> = self
            .requires
            .iter()
            .filter_map(|required| map.node(required, None))
            .collect();
        let relevant_when = self.relevant_when.as_ref().and_then(|condition| {
            condition.map_decisions(|decision| map.node(decision, Some(NodeField::RelevantWhen)))
        });
        let due_by = self
            .due_by
            .as_ref()
            .and_then(|rule| map_rule(rule, NodeField::DueBy, map));
        let not_before = self
            .not_before
            .as_ref()
            .and_then(|rule| map_rule(rule, NodeField::NotBefore, map));
        let participations = map_participations(&self.participations, map);
        let resources: Vec<Resource<B>> = self
            .resources
            .iter()
            .filter_map(|resource| map_resource(resource, map))
            .collect();
        let payload = map_payload(&self.payload, map);
        Node {
            key: map.own_key(&self.key),
            id: self.id.clone(),
            parent,
            title: self.title.clone(),
            description: self.description.clone(),
            weight: self.weight,
            requires: crate::collections::BoundedSet::new(requires).unwrap_or_default(),
            relevant_when,
            due_by,
            not_before,
            participations,
            resources,
            payload,
        }
    }
}

fn map_rule<A: References, B: References>(
    rule: &DateRule<A>,
    field: NodeField,
    map: &mut impl MapReferences<A, B>,
) -> Option<DateRule<B>> {
    let mut sources = Vec::new();
    for source in rule.sources.as_set() {
        match source {
            DateSource::CreatedAt => sources.push(DateSource::CreatedAt),
            DateSource::Node(node) => {
                sources.extend(map.node(node, Some(field)).map(DateSource::Node));
            }
        }
    }
    Some(DateRule {
        direction: rule.direction,
        sources: crate::collections::OneOrMany::new(sources).ok()?,
        offset: rule.offset,
    })
}

/// Two kinds that translate to one collapse into one entry, which the caller of an
/// insertion detects by the count.
fn map_participations<A: References, B: References>(
    participations: &Participations<A>,
    map: &mut impl MapReferences<A, B>,
) -> Participations<B> {
    let mut mapped = BTreeMap::new();
    for (kind, source) in participations.as_map() {
        let kind = map.kind(kind);
        let source = match source {
            ParticipationSource::Role(role) => map.role(role, None).map(ParticipationSource::Role),
            ParticipationSource::Identities(identities) => {
                Some(ParticipationSource::Identities(identities.clone()))
            }
        };
        if let (Some(kind), Some(source)) = (kind, source) {
            mapped.insert(kind, source);
        }
    }
    Participations::try_from(mapped).unwrap_or_default()
}

fn map_resource<A: References, B: References>(
    resource: &Resource<A>,
    map: &mut impl MapReferences<A, B>,
) -> Option<Resource<B>> {
    let key = map.resource_key(&resource.key);
    let content = match &resource.content {
        ResourceContent::Tip(text) => ResourceContent::Tip(text.clone()),
        ResourceContent::Template(url) => ResourceContent::Template(url.clone()),
        ResourceContent::Example(url) => ResourceContent::Example(url.clone()),
        ResourceContent::Reference(url) => ResourceContent::Reference(url.clone()),
        ResourceContent::MessageDraft(template) => {
            ResourceContent::MessageDraft(map_template(template, &resource.key, map)?)
        }
    };
    Some(Resource {
        key,
        title: resource.title.clone(),
        content,
    })
}

fn map_template<A: References, B: References>(
    template: &MessageTemplate<A>,
    resource: &A::AttachmentKey,
    map: &mut impl MapReferences<A, B>,
) -> Option<MessageTemplate<B>> {
    let translated = template.map_references(|segment| match segment {
        Segment::Text(literal) => Some(Segment::Text(literal.clone())),
        Segment::Journey(field) => Some(Segment::Journey(*field)),
        Segment::RoleName(role) => map.role(role, None).map(Segment::RoleName),
        Segment::Answer(node) => map.node(node, None).map(Segment::Answer),
    });
    match translated {
        Ok(template) => Some(template),
        Err(TemplateRefusal::Unmapped) => None,
        Err(TemplateRefusal::TooLong(bytes)) => {
            map.draft_too_long(resource, bytes);
            None
        }
    }
}

fn map_payload<A: References, B: References>(
    payload: &Payload<A>,
    map: &mut impl MapReferences<A, B>,
) -> Payload<B> {
    match payload {
        Payload::Decision(decision) => Payload::Decision(Decision {
            prompt: decision.prompt.clone(),
            help: decision.help.clone(),
            answer: match &decision.answer {
                AnswerSpec::Boolean => AnswerSpec::Boolean,
                AnswerSpec::SingleChoice(choices) => AnswerSpec::SingleChoice(choices.clone()),
                AnswerSpec::MultiChoice(choices) => AnswerSpec::MultiChoice(choices.clone()),
                AnswerSpec::Text => AnswerSpec::Text,
                AnswerSpec::Date { feeds_milestone } => AnswerSpec::Date {
                    feeds_milestone: feeds_milestone
                        .as_ref()
                        .and_then(|node| map.node(node, Some(NodeField::FeedsMilestone))),
                },
                AnswerSpec::Identity { fills_role } => AnswerSpec::Identity {
                    fills_role: fills_role
                        .as_ref()
                        .and_then(|role| map.role(role, Some(NodeField::FillsRole))),
                },
                AnswerSpec::IdentityList { fills_role } => AnswerSpec::IdentityList {
                    fills_role: fills_role
                        .as_ref()
                        .and_then(|role| map.role(role, Some(NodeField::FillsRole))),
                },
            },
        }),
        Payload::Deliverable(deliverable) => Payload::Deliverable(deliverable.clone()),
        Payload::Action(action) => Payload::Action(action.clone()),
        Payload::Milestone(milestone) => Payload::Milestone(milestone.clone()),
        Payload::Group(group) => Payload::Group(Group {
            opens_at: group
                .opens_at
                .as_ref()
                .and_then(|node| map.node(node, Some(NodeField::OpensAt))),
            closes_at: group
                .closes_at
                .as_ref()
                .and_then(|node| map.node(node, Some(NodeField::ClosesAt))),
            gates: group.gates,
            closes: group.closes,
        }),
    }
}
