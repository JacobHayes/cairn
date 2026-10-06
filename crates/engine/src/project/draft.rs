//! A10, G3: a message draft rendered with journey context, over its parsed segments (stored
//! parsed, so rendering never re-parses). Journey fields come from the journey's header (its
//! `created_at` as the deployment-local day it was created, A9) and the host's link to it;
//! a role's names from its members now (E3: the filling decision's answer while in effect,
//! else the direct fill) through the deployment's entities; an answer from the decision's
//! answer while in effect, choices by their labels. Whatever has no value is a visible marker
//! naming the placeholder as written; rendering never fails.
//!
//! Cost: one pass over the template's segments, each a lookup.

use std::collections::BTreeSet;

use cairn_schema::{
    AnswerSpec, AnswerValue, Deployment, EntityKey, JourneyField, JourneyHeader, JourneyStatus,
    KeyRefs, MessageTemplate, NodeKey, Payload, RenderedDraft, RenderedSegment, Segment, Url,
};

use super::DerivedJourney;

/// What a draft renders against besides the derived journey.
#[derive(Clone, Copy, Debug)]
pub struct DraftContext<'a> {
    /// The journey's own fields.
    pub header: &'a JourneyHeader,
    /// The link to the journey, which only the host knows; none renders a marker.
    pub url: Option<&'a Url>,
    /// The deployment, whose entities name role members and entity answers.
    pub deployment: &'a Deployment,
}

impl DerivedJourney<'_> {
    /// A10, G3: the draft with each placeholder filled from journey context, or marked when
    /// there is nothing to fill it with.
    #[must_use]
    pub fn render_draft(
        &self,
        template: &MessageTemplate<KeyRefs>,
        context: &DraftContext<'_>,
    ) -> RenderedDraft {
        let segments = template
            .segments()
            .iter()
            .map(|segment| {
                match segment {
                    Segment::Text(text) => Some(text.clone()),
                    Segment::Journey(field) => journey_field(*field, context),
                    Segment::RoleName(role) => {
                        let members = self.derived.participation().members(role);
                        (!members.is_empty()).then(|| names(members, context.deployment))
                    }
                    Segment::Answer(decision) => self.answer_text(decision, context.deployment),
                }
                .map_or_else(
                    || RenderedSegment::Missing(self.placeholder(segment)),
                    RenderedSegment::Text,
                )
            })
            .collect();
        RenderedDraft { segments }
    }

    /// The decision's answer while in effect, as text.
    fn answer_text(&self, decision: &NodeKey, deployment: &Deployment) -> Option<String> {
        let answer = self
            .derived
            .relevance()
            .answer_in_effect(self.graph.document(), decision)?;
        let label = |id: &cairn_schema::Slug| {
            let choices = match &self.graph.node(decision)?.payload {
                Payload::Decision(found) => match &found.answer {
                    AnswerSpec::SingleChoice(choices) | AnswerSpec::MultiChoice(choices) => choices,
                    _ => return None,
                },
                _ => return None,
            };
            let choice = choices.as_slice().iter().find(|choice| choice.id == *id)?;
            Some(choice.title.as_ref()?.as_str().to_owned())
        };
        let choice = |id: &cairn_schema::Slug| label(id).unwrap_or_else(|| id.as_str().to_owned());
        Some(match answer {
            AnswerValue::Boolean(true) => "yes".to_owned(),
            AnswerValue::Boolean(false) => "no".to_owned(),
            AnswerValue::SingleChoice(id) => choice(id),
            AnswerValue::MultiChoice(ids) => ids.iter().map(choice).collect::<Vec<_>>().join(", "),
            AnswerValue::Text(text) => text.as_str().to_owned(),
            AnswerValue::Date(date) => date.to_string(),
            AnswerValue::Entity(entity) => names(&BTreeSet::from([entity.clone()]), deployment),
            AnswerValue::EntityList(entities) => {
                names(&entities.iter().cloned().collect(), deployment)
            }
        })
    }

    /// The placeholder as a route file writes it: roles by id and decisions by path.
    fn placeholder(&self, segment: &Segment<KeyRefs>) -> String {
        match segment {
            Segment::Text(text) => text.clone(),
            Segment::Journey(field) => {
                let written = MessageTemplate::<KeyRefs>::render_segment(&Segment::Journey(*field));
                written.trim_matches(|c| c == '{' || c == '}').to_owned()
            }
            Segment::RoleName(role) => {
                let id = self.graph.document().roles.get(role).map_or_else(
                    || role.as_str().to_owned(),
                    |found| found.id.as_str().to_owned(),
                );
                format!("roles.{id}.name")
            }
            Segment::Answer(decision) => {
                let path = self
                    .graph
                    .tree()
                    .path(decision)
                    .map_or_else(|| decision.as_str().to_owned(), ToString::to_string);
                format!("answers.{path}")
            }
        }
    }
}

/// A journey field's value, none when it has none.
fn journey_field(field: JourneyField, context: &DraftContext<'_>) -> Option<String> {
    let header = context.header;
    match field {
        JourneyField::Name => Some(header.name.as_str().to_owned()),
        JourneyField::Description => header
            .description
            .as_ref()
            .map(|text| text.as_str().to_owned()),
        JourneyField::CreatedAt => Some(header.created_on.to_string()),
        JourneyField::Url => context.url.map(|url| url.as_str().to_owned()),
        JourneyField::Status => Some(
            match header.status {
                JourneyStatus::Active => "active",
                JourneyStatus::Completed => "completed",
                JourneyStatus::Archived => "archived",
            }
            .to_owned(),
        ),
    }
}

/// The entities' names, each key read through aliases (E6), in key order, joined; a key the
/// deployment does not know stands for itself.
fn names(entities: &BTreeSet<EntityKey>, deployment: &Deployment) -> String {
    entities
        .iter()
        .map(|key| crate::entity::resolve(deployment, key).unwrap_or(key))
        .collect::<BTreeSet<&EntityKey>>()
        .into_iter()
        .map(|key| {
            deployment.entities.get(key).map_or_else(
                || key.as_str().to_owned(),
                |entity| entity.name.as_str().to_owned(),
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}
