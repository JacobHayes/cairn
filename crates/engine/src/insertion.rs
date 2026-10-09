//! What segment insertion shares with whatever later moves an insertion's keys (B13): the
//! pure function that mints every key an insertion creates from the insertion's key and the
//! key in the segment, and the translation of a node's references from the segment's keys to
//! the graph's. Both are pure, so a proposal's preview and its apply write the same keys.
//!
//! Cost: one pass over a node's fields, each reference one lookup; a condition is walked with
//! an explicit stack (PRACTICES, No recursion).

use std::collections::BTreeMap;

use cairn_schema::id::fnv1a_body;
use cairn_schema::{
    AnswerSpec, AttachmentKey, Clause, Comparison, Condition, DateRule, DateSource, Decision,
    Group, InsertionKey, KeyRefs, KindKey, Membership, MessageTemplate, Node, NodeKey, OneOrMany,
    ParticipationSource, Participations, Payload, Prefixed, Resource, ResourceContent, RoleKey,
    Segment,
};

/// B13: the key of type `K` an insertion mints for the object that has `segment_key` in the
/// segment: the prefix and 16 hex digits of the FNV-1a hash of the insertion key, a tag for
/// the object's kind, and the segment key. Two insertions of one version into one graph have
/// different keys, so all their minted keys differ.
///
/// # Panics
///
/// Never: the body is 16 hex digits, a slug within the key length.
#[must_use]
pub(crate) fn mint<K: Prefixed>(insertion: &InsertionKey, segment_key: &str) -> K {
    let tag = K::PREFIX.trim_end_matches('_');
    let body = fnv1a_body(&[
        insertion.as_str().as_bytes(),
        &[0],
        tag.as_bytes(),
        &[0],
        segment_key.as_bytes(),
    ]);
    match K::from_body(&body) {
        Ok(key) => key,
        Err(error) => unreachable!("a minted body is a slug: {error}"),
    }
}

/// The graph's key for each key a segment node refers to.
pub(crate) struct Remap<'a> {
    /// A node key in the segment, to its key in the graph.
    pub node: &'a dyn Fn(&NodeKey) -> NodeKey,
    /// A role key in the segment, to its key in the graph.
    pub role: &'a dyn Fn(&RoleKey) -> RoleKey,
    /// A participation kind key in the segment, to its key in the graph (`owner` to itself).
    pub kind: &'a dyn Fn(&KindKey) -> KindKey,
    /// A resource key in the segment, to its key in the graph.
    pub resource: &'a dyn Fn(&AttachmentKey) -> AttachmentKey,
}

impl Remap<'_> {
    /// The node as the graph holds it: its own key, parent, edges, and every reference
    /// translated.
    ///
    /// # Errors
    ///
    /// When a message draft, with its placeholders translated to longer keys, passes the body
    /// limit.
    ///
    /// # Panics
    ///
    /// Never: translation keeps every count a node already had within its limits.
    pub(crate) fn node(&self, node: &Node<KeyRefs>) -> Result<Node<KeyRefs>, String> {
        let requires = node.requires.iter().map(|key| (self.node)(key));
        let resources: Vec<Resource<KeyRefs>> = node
            .resources
            .iter()
            .map(|resource| self.resource(resource))
            .collect::<Result<_, _>>()?;
        Ok(Node {
            key: (self.node)(&node.key),
            id: node.id.clone(),
            parent: node.parent.as_ref().map(|parent| (self.node)(parent)),
            title: node.title.clone(),
            description: node.description.clone(),
            weight: node.weight,
            requires: match cairn_schema::BoundedSet::new(requires) {
                Ok(requires) => requires,
                Err(error) => unreachable!("edges are as many as before: {error}"),
            },
            relevant_when: node
                .relevant_when
                .as_ref()
                .map(|condition| self.condition(condition)),
            due_by: node.due_by.as_ref().map(|rule| self.rule(rule)),
            not_before: node.not_before.as_ref().map(|rule| self.rule(rule)),
            participations: self.participations(&node.participations),
            resources: resources.into_iter().collect(),
            payload: self.payload(&node.payload),
        })
    }

    fn rule(&self, rule: &DateRule<KeyRefs>) -> DateRule<KeyRefs> {
        let sources = rule.sources.as_set().iter().map(|source| match source {
            DateSource::CreatedAt => DateSource::<KeyRefs>::CreatedAt,
            DateSource::Node(key) => DateSource::Node((self.node)(key)),
        });
        DateRule {
            direction: rule.direction,
            sources: match OneOrMany::new(sources.collect::<Vec<_>>()) {
                Ok(sources) => sources,
                Err(error) => unreachable!("sources are as many as before: {error}"),
            },
            offset: rule.offset,
        }
    }

    fn participations(&self, participations: &Participations<KeyRefs>) -> Participations<KeyRefs> {
        let map: BTreeMap<KindKey, ParticipationSource<KeyRefs>> = participations
            .as_map()
            .iter()
            .map(|(kind, source)| {
                let source = match source {
                    ParticipationSource::Role(role) => ParticipationSource::Role((self.role)(role)),
                    ParticipationSource::Entities(entities) => {
                        ParticipationSource::Entities(entities.clone())
                    }
                };
                ((self.kind)(kind), source)
            })
            .collect();
        match Participations::try_from(map) {
            Ok(participations) => participations,
            Err(error) => unreachable!("kinds are as many as before: {error}"),
        }
    }

    fn resource(&self, resource: &Resource<KeyRefs>) -> Result<Resource<KeyRefs>, String> {
        let content = match &resource.content {
            ResourceContent::MessageDraft(template) => {
                ResourceContent::MessageDraft(self.template(template).map_err(|error| {
                    format!(
                        "message draft {} once its keys are minted: {error}",
                        resource.key
                    )
                })?)
            }
            other => other.clone(),
        };
        Ok(Resource {
            key: (self.resource)(&resource.key),
            title: resource.title.clone(),
            content,
        })
    }

    /// A message draft with its placeholders translated, written and read back so the
    /// template grammar stays the schema's.
    fn template(
        &self,
        template: &MessageTemplate<KeyRefs>,
    ) -> Result<MessageTemplate<KeyRefs>, impl std::fmt::Display> {
        let text: String = template
            .segments()
            .iter()
            .map(|segment| {
                let translated: Segment<KeyRefs> = match segment {
                    Segment::Text(literal) => Segment::Text(literal.clone()),
                    Segment::Journey(field) => Segment::Journey(*field),
                    Segment::RoleName(role) => Segment::RoleName((self.role)(role)),
                    Segment::Answer(key) => Segment::Answer((self.node)(key)),
                };
                MessageTemplate::<KeyRefs>::render_segment(&translated)
            })
            .collect();
        text.parse()
    }

    fn payload(&self, payload: &Payload<KeyRefs>) -> Payload<KeyRefs> {
        match payload {
            Payload::Decision(decision) => Payload::Decision(Decision {
                prompt: decision.prompt.clone(),
                help: decision.help.clone(),
                answer: match &decision.answer {
                    AnswerSpec::Date { feeds_milestone } => AnswerSpec::Date {
                        feeds_milestone: feeds_milestone.as_ref().map(|key| (self.node)(key)),
                    },
                    AnswerSpec::Entity { fills_role } => AnswerSpec::Entity {
                        fills_role: fills_role.as_ref().map(|role| (self.role)(role)),
                    },
                    AnswerSpec::EntityList { fills_role } => AnswerSpec::EntityList {
                        fills_role: fills_role.as_ref().map(|role| (self.role)(role)),
                    },
                    other => other.clone(),
                },
            }),
            Payload::Group(group) => Payload::Group(Group {
                opens_at: group.opens_at.as_ref().map(|key| (self.node)(key)),
                closes_at: group.closes_at.as_ref().map(|key| (self.node)(key)),
                gates: group.gates,
                closes: group.closes,
            }),
            other => other.clone(),
        }
    }

    /// A condition with its decisions translated, built bottom up with an explicit stack.
    fn condition(&self, condition: &Condition<KeyRefs>) -> Condition<KeyRefs> {
        enum Frame<'c> {
            Visit(&'c Clause<KeyRefs>),
            Build(&'c Clause<KeyRefs>),
        }
        let mut frames = vec![Frame::Visit(condition.root())];
        let mut built: Vec<Clause<KeyRefs>> = Vec::new();
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Visit(clause) => match clause {
                    Clause::All(children) | Clause::Any(children) => {
                        frames.push(Frame::Build(clause));
                        frames.extend(children.iter().rev().map(Frame::Visit));
                    }
                    Clause::Not(child) => {
                        frames.push(Frame::Build(clause));
                        frames.push(Frame::Visit(child));
                    }
                    leaf => built.push(self.leaf(leaf)),
                },
                Frame::Build(clause) => {
                    let count = match clause {
                        Clause::All(children) | Clause::Any(children) => children.len(),
                        _ => 1,
                    };
                    assert!(built.len() >= count, "children are built first");
                    let mut children = built.split_off(built.len() - count);
                    built.push(match clause {
                        Clause::All(_) => Clause::All(children),
                        Clause::Any(_) => Clause::Any(children),
                        _ => match children.pop() {
                            Some(child) => Clause::Not(Box::new(child)),
                            None => unreachable!("a negation has its child"),
                        },
                    });
                }
            }
        }
        let root = built.pop();
        assert!(built.is_empty(), "every clause folds into the root");
        match root.map(Condition::new) {
            Some(Ok(condition)) => condition,
            Some(Err(error)) => panic!("a condition keeps its shape and limits: {error}"),
            None => unreachable!("a condition has a root"),
        }
    }

    fn leaf(&self, clause: &Clause<KeyRefs>) -> Clause<KeyRefs> {
        let compared = |comparison: &Comparison<KeyRefs>| Comparison {
            decision: (self.node)(&comparison.decision),
            value: comparison.value.clone(),
        };
        match clause {
            Clause::Equals(comparison) => Clause::Equals(compared(comparison)),
            Clause::NotEquals(comparison) => Clause::NotEquals(compared(comparison)),
            Clause::Contains(comparison) => Clause::Contains(compared(comparison)),
            Clause::In(membership) => Clause::In(Membership {
                decision: (self.node)(&membership.decision),
                values: membership.values.clone(),
            }),
            Clause::Answered(decision) => Clause::Answered((self.node)(decision)),
            Clause::All(_) | Clause::Any(_) | Clause::Not(_) => {
                unreachable!("combinations are folded")
            }
        }
    }
}
