//! Attachments (A10, G1, G2): one model behind resources (route-authored guidance on a
//! node) and annotations (journey-authored notes and links on a node or the journey),
//! distinguished by scope and type. Message drafts are stored as parsed segments, so their
//! placeholders are references like any other and rendering never re-parses.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::id::{AttachmentKey, NodeKey, UserId};
use crate::limits::Limit;
use crate::refs::{References, key_absent};
use crate::text::{Markdown, Title, Url};

/// Whether an attachment is route guidance or journey annotation (G1).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentScope {
    /// A resource: route-authored guidance, part of the graph's structure.
    Resource,
    /// A note or link: journey-authored annotation, part of the journey's state.
    Annotation,
}

/// A journey field a message draft can show (A10).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum JourneyField {
    /// `{{journey.name}}`.
    Name,
    /// `{{journey.description}}`.
    Description,
    /// `{{journey.created_at}}`.
    CreatedAt,
    /// `{{journey.url}}`.
    Url,
    /// `{{journey.status}}`.
    Status,
}

impl JourneyField {
    const ALL: [JourneyField; 5] = [
        JourneyField::Name,
        JourneyField::Description,
        JourneyField::CreatedAt,
        JourneyField::Url,
        JourneyField::Status,
    ];

    fn name(self) -> &'static str {
        match self {
            JourneyField::Name => "name",
            JourneyField::Description => "description",
            JourneyField::CreatedAt => "created_at",
            JourneyField::Url => "url",
            JourneyField::Status => "status",
        }
    }
}

/// One piece of a message draft.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Segment<R: References> {
    /// Literal text.
    Text(String),
    /// A journey field.
    Journey(JourneyField),
    /// The names of the entities filling a role.
    RoleName(R::Role),
    /// A decision's answer.
    Answer(R::Node),
}

/// A message draft: text with journey-context placeholders, written as a template
/// (`Hello {{roles.eval_owner.name}}, about {{journey.name}}: {{answers.testing/scope}}`)
/// and held as parsed segments. In the resolved form the placeholders name keys.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MessageTemplate<R: References>(Vec<Segment<R>>);

/// Why a message template was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplateError(String);

impl fmt::Display for TemplateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "message draft: {}", self.0)
    }
}

impl std::error::Error for TemplateError {}

impl<R: References> MessageTemplate<R> {
    /// The segments, in order.
    #[must_use]
    pub fn segments(&self) -> &[Segment<R>] {
        &self.0
    }

    fn parse_placeholder(inner: &str) -> Result<Segment<R>, TemplateError> {
        let inner = inner.trim();
        if let Some(field) = inner.strip_prefix("journey.") {
            let found = JourneyField::ALL
                .into_iter()
                .find(|candidate| candidate.name() == field);
            return found
                .map(Segment::Journey)
                .ok_or_else(|| TemplateError(format!("unknown journey field {field:?}")));
        }
        if let Some(role) = inner
            .strip_prefix("roles.")
            .and_then(|rest| rest.strip_suffix(".name"))
        {
            return role
                .parse()
                .map(Segment::RoleName)
                .map_err(|error| TemplateError(format!("role {role:?}: {error}")));
        }
        if let Some(decision) = inner.strip_prefix("answers.") {
            return decision
                .parse()
                .map(Segment::Answer)
                .map_err(|error| TemplateError(format!("answer {decision:?}: {error}")));
        }
        Err(TemplateError(format!(
            "unknown placeholder {{{{{inner}}}}} (expected journey.<field>, roles.<role>.name, or answers.<decision>)"
        )))
    }
}

impl<R: References> std::str::FromStr for MessageTemplate<R> {
    type Err = TemplateError;

    fn from_str(text: &str) -> Result<Self, TemplateError> {
        Limit::BodyBytes
            .check(text.len())
            .map_err(|exceeded| TemplateError(exceeded.to_string()))?;
        let mut segments = Vec::new();
        let mut rest = text;
        while let Some(open) = rest.find("{{") {
            let (literal, after) = rest.split_at(open);
            if !literal.is_empty() {
                segments.push(Segment::Text(literal.to_owned()));
            }
            let after = &after[2..];
            let Some(close) = after.find("}}") else {
                return Err(TemplateError("a `{{` has no closing `}}`".to_owned()));
            };
            segments.push(Self::parse_placeholder(&after[..close])?);
            rest = &after[close + 2..];
        }
        if !rest.is_empty() {
            segments.push(Segment::Text(rest.to_owned()));
        }
        if segments.is_empty() {
            return Err(TemplateError("empty".to_owned()));
        }
        Ok(Self(segments))
    }
}

impl<R: References> MessageTemplate<R> {
    /// One segment as the template writes it.
    #[must_use]
    pub fn render_segment(segment: &Segment<R>) -> String {
        match segment {
            Segment::Text(text) => text.clone(),
            Segment::Journey(field) => format!("{{{{journey.{}}}}}", field.name()),
            Segment::RoleName(role) => format!("{{{{roles.{role}.name}}}}"),
            Segment::Answer(decision) => format!("{{{{answers.{decision}}}}}"),
        }
    }
}

impl<R: References> fmt::Display for MessageTemplate<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for segment in &self.0 {
            formatter.write_str(&Self::render_segment(segment))?;
        }
        Ok(())
    }
}

impl<R: References> Serialize for MessageTemplate<R> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de, R: References> Deserialize<'de> for MessageTemplate<R> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

impl<R: References> JsonSchema for MessageTemplate<R> {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "MessageTemplate".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "description": "A message draft with placeholders: {{journey.<name|description|created_at|url|status>}}, {{roles.<role>.name}}, {{answers.<decision>}}.",
            "minLength": 1,
            "maxLength": crate::limits::BODY_BYTES_MAX,
        })
    }
}

/// What a resource holds (A10; PRD glossary, Resource: tip, template, example, reference,
/// message draft).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceContent<R: References> {
    /// Markdown guidance.
    Tip(Markdown),
    /// A link to a template to start from.
    Template(Url),
    /// A link to an example of finished work.
    Example(Url),
    /// A link to reference material.
    Reference(Url),
    /// A message to copy and send.
    MessageDraft(MessageTemplate<R>),
}

/// Route-authored guidance on a node (A10), keyed so upgrades diff it (B7).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(try_from = "ResourceWire<R>", into = "ResourceWire<R>", bound = "")]
#[schemars(with = "ResourceWire<R>", bound = "R: References")]
pub struct Resource<R: References> {
    /// The resource's key: optional in a file.
    pub key: R::AttachmentKey,
    /// A label.
    pub title: Option<Title>,
    /// The content.
    pub content: ResourceContent<R>,
}

/// The written form of a resource: its key and title, and exactly one content field
/// named after its type.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, bound = "")]
#[schemars(bound = "R: References", rename = "Resource{R}", transform = resource_schema)]
pub struct ResourceWire<R: References> {
    #[serde(skip_serializing_if = "key_absent")]
    key: R::AttachmentKey,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<Title>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Markdown")]
    tip: Option<Markdown>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Url")]
    template: Option<Url>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Url")]
    example: Option<Url>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Url")]
    reference: Option<Url>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "MessageTemplate<R>")]
    message_draft: Option<MessageTemplate<R>>,
}

fn resource_schema(schema: &mut schemars::Schema) {
    crate::serde_util::exactly_one_of(
        schema,
        &["tip", "template", "example", "reference", "message_draft"],
    );
}

fn annotation_schema(schema: &mut schemars::Schema) {
    crate::serde_util::exactly_one_of(schema, &["note", "artifact", "reference", "conversation"]);
}

/// Why an attachment's written form was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentError(&'static str);

impl fmt::Display for ContentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "an attachment has exactly one of: {}", self.0)
    }
}

impl std::error::Error for ContentError {}

fn exactly_one<T>(fields: &'static str, options: Vec<Option<T>>) -> Result<T, ContentError> {
    let mut present = options.into_iter().flatten();
    match (present.next(), present.next()) {
        (Some(only), None) => Ok(only),
        _ => Err(ContentError(fields)),
    }
}

impl<R: References> TryFrom<ResourceWire<R>> for Resource<R> {
    type Error = ContentError;

    fn try_from(wire: ResourceWire<R>) -> Result<Self, ContentError> {
        let content = exactly_one(
            "tip, template, example, reference, message_draft",
            vec![
                wire.tip.map(ResourceContent::Tip),
                wire.template.map(ResourceContent::Template),
                wire.example.map(ResourceContent::Example),
                wire.reference.map(ResourceContent::Reference),
                wire.message_draft.map(ResourceContent::MessageDraft),
            ],
        )?;
        Ok(Self {
            key: wire.key,
            title: wire.title,
            content,
        })
    }
}

impl<R: References> From<Resource<R>> for ResourceWire<R> {
    fn from(resource: Resource<R>) -> Self {
        let mut wire = ResourceWire {
            key: resource.key,
            title: resource.title,
            tip: None,
            template: None,
            example: None,
            reference: None,
            message_draft: None,
        };
        match resource.content {
            ResourceContent::Tip(body) => wire.tip = Some(body),
            ResourceContent::Template(url) => wire.template = Some(url),
            ResourceContent::Example(url) => wire.example = Some(url),
            ResourceContent::Reference(url) => wire.reference = Some(url),
            ResourceContent::MessageDraft(template) => wire.message_draft = Some(template),
        }
        wire
    }
}

impl<R: References> Resource<R> {
    /// Resources are route guidance.
    pub const SCOPE: AttachmentScope = AttachmentScope::Resource;
}

impl crate::collections::HasKey for Resource<crate::refs::KeyRefs> {
    type Key = AttachmentKey;

    fn key(&self) -> &AttachmentKey {
        &self.key
    }
}

/// What a note or link holds (G1: notes, and links typed artifact, reference, or
/// conversation).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AnnotationContent {
    /// A markdown note.
    Note(Markdown),
    /// A link designated as a deliverable's output (G2).
    Artifact(Url),
    /// A link to reference material.
    Reference(Url),
    /// A link to a conversation.
    Conversation(Url),
}

/// A note or link as its author writes it (G1): the part an add or edit carries.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(try_from = "AnnotationWire", into = "AnnotationWire")]
#[schemars(with = "AnnotationWire")]
pub struct AnnotationBody {
    /// The annotation's key.
    pub key: AttachmentKey,
    /// The node it annotates, or none for the journey itself.
    pub node: Option<NodeKey>,
    /// A label.
    pub title: Option<Title>,
    /// The content.
    pub content: AnnotationContent,
}

/// The written form of an annotation body: exactly one content field named after its type.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AnnotationBody", transform = annotation_schema)]
pub struct AnnotationWire {
    key: AttachmentKey,
    #[serde(skip_serializing_if = "Option::is_none")]
    node: Option<NodeKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<Title>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Markdown")]
    note: Option<Markdown>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Url")]
    artifact: Option<Url>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Url")]
    reference: Option<Url>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Url")]
    conversation: Option<Url>,
}

impl TryFrom<AnnotationWire> for AnnotationBody {
    type Error = ContentError;

    fn try_from(wire: AnnotationWire) -> Result<Self, ContentError> {
        let content = exactly_one(
            "note, artifact, reference, conversation",
            vec![
                wire.note.map(AnnotationContent::Note),
                wire.artifact.map(AnnotationContent::Artifact),
                wire.reference.map(AnnotationContent::Reference),
                wire.conversation.map(AnnotationContent::Conversation),
            ],
        )?;
        Ok(Self {
            key: wire.key,
            node: wire.node,
            title: wire.title,
            content,
        })
    }
}

impl From<AnnotationBody> for AnnotationWire {
    fn from(body: AnnotationBody) -> Self {
        let mut wire = AnnotationWire {
            key: body.key,
            node: body.node,
            title: body.title,
            note: None,
            artifact: None,
            reference: None,
            conversation: None,
        };
        match body.content {
            AnnotationContent::Note(text) => wire.note = Some(text),
            AnnotationContent::Artifact(url) => wire.artifact = Some(url),
            AnnotationContent::Reference(url) => wire.reference = Some(url),
            AnnotationContent::Conversation(url) => wire.conversation = Some(url),
        }
        wire
    }
}

/// A journey-authored note or link on a node or on the journey itself (G1), attributed and
/// timestamped from the patch that added it.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Annotation {
    /// What its author wrote.
    pub body: AnnotationBody,
    /// Who added it.
    pub created_by: UserId,
    /// When it was added.
    pub created_at: jiff::Timestamp,
    /// When it was last edited, if ever.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<jiff::Timestamp>,
}

impl Annotation {
    /// Notes and links are journey annotation.
    pub const SCOPE: AttachmentScope = AttachmentScope::Annotation;

    /// True for an artifact link (G2), which a `requires_artifact` deliverable's guard
    /// looks for on the node itself.
    #[must_use]
    pub fn is_artifact(&self) -> bool {
        matches!(self.body.content, AnnotationContent::Artifact(_))
    }
}

impl crate::collections::HasKey for Annotation {
    type Key = AttachmentKey;

    fn key(&self) -> &AttachmentKey {
        &self.body.key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refs::{FileRefs, KeyRefs};

    #[test]
    fn message_templates_round_trip_through_segments() {
        let text =
            "Hello {{roles.eval_owner.name}}, about {{journey.name}}: {{answers.testing/scope}}.";
        let template: MessageTemplate<FileRefs> = text.parse().unwrap();
        assert_eq!(template.segments().len(), 7);
        assert!(
            matches!(&template.segments()[1], Segment::RoleName(role) if role.as_str() == "eval_owner")
        );
        assert_eq!(template.to_string(), text);
        let resolved: MessageTemplate<KeyRefs> = "{{answers.n_scope}} for {{roles.r_owner.name}}"
            .parse()
            .unwrap();
        assert_eq!(resolved.segments().len(), 3);
    }

    #[test]
    fn message_templates_reject_unknown_placeholders() {
        for bad in [
            "{{journey.secret}}",
            "{{roles.owner}}",
            "{{answers.Bad Path}}",
            "open {{ only",
            "{{other}}",
        ] {
            assert!(bad.parse::<MessageTemplate<FileRefs>>().is_err(), "{bad}");
        }
        // In the resolved form a placeholder must name a key.
        assert!(
            "{{answers.testing/scope}}"
                .parse::<MessageTemplate<KeyRefs>>()
                .is_err()
        );
    }

    #[test]
    fn a_resource_has_exactly_one_content_field() {
        let tip: Resource<FileRefs> =
            serde_json::from_value(serde_json::json!({"tip": "Ask early."})).unwrap();
        assert_eq!(tip.key, None);
        for bad in [
            serde_json::json!({"title": "Nothing"}),
            serde_json::json!({"tip": "x", "template": "https://example.org/t"}),
            serde_json::json!({"tip": "x", "body": "y"}),
        ] {
            assert!(
                serde_json::from_value::<Resource<FileRefs>>(bad.clone()).is_err(),
                "{bad}"
            );
        }
        // The resolved form requires the key.
        assert!(
            serde_json::from_value::<Resource<KeyRefs>>(serde_json::json!({"tip": "x"})).is_err()
        );
    }
}
