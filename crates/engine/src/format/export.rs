//! Exporting a route version or draft to the file document (A13; ARCHITECTURE, File format):
//! every key kept, every reference written as a path or an id, and everything in one order
//! (nodes by path, so a parent precedes its children; roles and kinds by id; a node's fields
//! in the schema's order), so two exports of one graph are the same bytes and an edit shows
//! as a small version-control diff.
//!
//! Cost at the limits: one tree build (O(n log n)) and one pass over the nodes, each
//! reference a map lookup.

use std::collections::BTreeMap;

use cairn_schema::{
    AnswerSpec, BoundedVec, Clause, Comparison, Condition, DateRule, DateSource, Decision,
    FileRefs, FormatVersion, Group, KeyRefs, KindKey, Markdown, Membership, MessageTemplate, Node,
    NodeKey, OneOrMany, ParticipationKind, ParticipationSource, Participations, Path, Payload,
    Resource, ResourceContent, Role, RoleKey, RouteFile, RouteId, Segment, Slug, Title,
    VersionNumber,
};

use crate::graph::Graph;

/// What a route file names besides its graph: the route and the version it extends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteHeading {
    /// The route's id.
    pub route: RouteId,
    /// The route's name.
    pub name: Title,
    /// What the route is for.
    pub description: Option<Markdown>,
    /// The published version the exported graph extends, if any.
    pub extends: Option<VersionNumber>,
}

/// A13: the file document for a route version or draft, deterministic.
///
/// # Panics
///
/// When the graph holds a journey's state, which no route has, or more than the limits its
/// type already bounds.
#[must_use]
pub fn export(graph: &Graph, heading: &RouteHeading) -> RouteFile {
    let document = graph.document();
    assert!(
        document.state.is_empty(),
        "a route graph holds no journey state"
    );
    let names = Names::of(graph);
    let mut nodes: Vec<(&Path, Node<FileRefs>)> = document
        .nodes
        .values()
        .map(|node| (names.path(&node.key), names.node(node)))
        .collect();
    nodes.sort_by_key(|(path, _)| *path);
    let mut roles: Vec<Role<FileRefs>> = document
        .roles
        .values()
        .map(|role| Role {
            key: Some(role.key.clone()),
            id: role.id.clone(),
            title: role.title.clone(),
            multi: role.multi,
        })
        .collect();
    roles.sort_by(|left, right| left.id.cmp(&right.id));
    let mut kinds: Vec<ParticipationKind<FileRefs>> = document
        .participation_kinds
        .values()
        .map(|kind| ParticipationKind {
            key: Some(kind.key.clone()),
            id: kind.id.clone(),
            title: kind.title.clone(),
            multi: kind.multi,
        })
        .collect();
    kinds.sort_by(|left, right| left.id.cmp(&right.id));
    let file = RouteFile {
        format: FormatVersion,
        route: heading.route.clone(),
        name: heading.name.clone(),
        description: heading.description.clone(),
        extends: heading.extends,
        default_owner: document.default_owner.as_ref().map(|role| names.role(role)),
        roles: bounded(roles),
        participation_kinds: bounded(kinds),
        nodes: bounded(nodes.into_iter().map(|(_, node)| node).collect()),
    };
    assert_eq!(file.nodes.len(), document.nodes.len());
    file
}

fn bounded<T, L: cairn_schema::collections::LimitOf>(values: Vec<T>) -> BoundedVec<T, L> {
    match BoundedVec::new(values) {
        Ok(values) => values,
        Err(error) => panic!("a graph's collections fit the file's bounds: {error}"),
    }
}

/// The path of every node and the id of every role and kind, by key.
struct Names<'a> {
    graph: &'a Graph,
    roles: BTreeMap<&'a RoleKey, &'a Slug>,
    kinds: BTreeMap<&'a KindKey, &'a Slug>,
}

impl<'a> Names<'a> {
    fn of(graph: &'a Graph) -> Self {
        let document = graph.document();
        Names {
            graph,
            roles: document
                .roles
                .values()
                .map(|role| (&role.key, &role.id))
                .collect(),
            kinds: document
                .participation_kinds
                .values()
                .map(|kind| (&kind.key, &kind.id))
                .collect(),
        }
    }

    fn path(&self, key: &NodeKey) -> &'a Path {
        match self.graph.tree().path(key) {
            Some(path) => path,
            None => unreachable!("every node of a valid graph has a path: {key}"),
        }
    }

    fn role(&self, key: &RoleKey) -> Slug {
        match self.roles.get(key) {
            Some(id) => (*id).clone(),
            None => unreachable!("every role a valid graph names exists: {key}"),
        }
    }

    fn kind(&self, key: &KindKey) -> Slug {
        if *key == KindKey::owner() {
            return cairn_schema::id::owner_kind_id();
        }
        match self.kinds.get(key) {
            Some(id) => (*id).clone(),
            None => unreachable!("every kind a valid graph names exists: {key}"),
        }
    }

    fn node(&self, node: &Node<KeyRefs>) -> Node<FileRefs> {
        let requires: Vec<Path> = node
            .requires
            .iter()
            .map(|key| self.path(key).clone())
            .collect();
        Node {
            key: Some(node.key.clone()),
            id: node.id.clone(),
            parent: node.parent.as_ref().map(|parent| self.path(parent).clone()),
            title: node.title.clone(),
            description: node.description.clone(),
            weight: node.weight,
            requires: match cairn_schema::BoundedSet::new(requires) {
                Ok(requires) => requires,
                Err(error) => unreachable!("paths are as many as keys: {error}"),
            },
            relevant_when: node
                .relevant_when
                .as_ref()
                .map(|condition| self.condition(condition)),
            due_by: node.due_by.as_ref().map(|rule| self.rule(rule)),
            not_before: node.not_before.as_ref().map(|rule| self.rule(rule)),
            participations: self.participations(&node.participations),
            resources: node
                .resources
                .iter()
                .map(|resource| self.resource(resource))
                .collect(),
            payload: self.payload(&node.payload),
        }
    }

    fn rule(&self, rule: &DateRule<KeyRefs>) -> DateRule<FileRefs> {
        let sources = rule.sources.as_set().iter().map(|source| match source {
            DateSource::CreatedAt => DateSource::<FileRefs>::CreatedAt,
            DateSource::Node(key) => DateSource::Node(self.path(key).clone()),
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

    fn participations(&self, participations: &Participations<KeyRefs>) -> Participations<FileRefs> {
        let map: BTreeMap<Slug, ParticipationSource<FileRefs>> = participations
            .as_map()
            .iter()
            .map(|(kind, source)| {
                let source = match source {
                    ParticipationSource::Role(role) => ParticipationSource::Role(self.role(role)),
                    ParticipationSource::Entities(entities) => {
                        ParticipationSource::Entities(entities.clone())
                    }
                };
                (self.kind(kind), source)
            })
            .collect();
        match Participations::try_from(map) {
            Ok(participations) => participations,
            Err(error) => unreachable!("kinds are as many as before: {error}"),
        }
    }

    fn resource(&self, resource: &Resource<KeyRefs>) -> Resource<FileRefs> {
        let content = match &resource.content {
            ResourceContent::Tip(text) => ResourceContent::Tip(text.clone()),
            ResourceContent::Template(url) => ResourceContent::Template(url.clone()),
            ResourceContent::Example(url) => ResourceContent::Example(url.clone()),
            ResourceContent::Reference(url) => ResourceContent::Reference(url.clone()),
            ResourceContent::MessageDraft(template) => {
                ResourceContent::MessageDraft(self.template(template))
            }
        };
        Resource {
            key: Some(resource.key.clone()),
            title: resource.title.clone(),
            content,
        }
    }

    /// A message draft with its placeholders named by id and path, written and read back so
    /// the template grammar stays the schema's.
    fn template(&self, template: &MessageTemplate<KeyRefs>) -> MessageTemplate<FileRefs> {
        let text: String = template
            .segments()
            .iter()
            .map(|segment| {
                let named: Segment<FileRefs> = match segment {
                    Segment::Text(literal) => Segment::Text(literal.clone()),
                    Segment::Journey(field) => Segment::Journey(*field),
                    Segment::RoleName(role) => Segment::RoleName(self.role(role)),
                    Segment::Answer(key) => Segment::Answer(self.path(key).clone()),
                };
                MessageTemplate::<FileRefs>::render_segment(&named)
            })
            .collect();
        match text.parse() {
            Ok(template) => template,
            Err(error) => panic!("an exported message draft reads back: {error}"),
        }
    }

    fn payload(&self, payload: &Payload<KeyRefs>) -> Payload<FileRefs> {
        let path = |key: &NodeKey| self.path(key).clone();
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
                        feeds_milestone: feeds_milestone.as_ref().map(path),
                    },
                    AnswerSpec::Entity { fills_role } => AnswerSpec::Entity {
                        fills_role: fills_role.as_ref().map(|role| self.role(role)),
                    },
                    AnswerSpec::EntityList { fills_role } => AnswerSpec::EntityList {
                        fills_role: fills_role.as_ref().map(|role| self.role(role)),
                    },
                },
            }),
            Payload::Deliverable(deliverable) => Payload::Deliverable(deliverable.clone()),
            Payload::Action(action) => Payload::Action(action.clone()),
            Payload::Milestone(milestone) => Payload::Milestone(milestone.clone()),
            Payload::Group(group) => Payload::Group(Group {
                opens_at: group.opens_at.as_ref().map(path),
                closes_at: group.closes_at.as_ref().map(path),
                gates: group.gates,
                closes: group.closes,
            }),
        }
    }

    /// A condition with its decisions named by path, built bottom up with an explicit stack
    /// (PRACTICES, No recursion).
    fn condition(&self, condition: &Condition<KeyRefs>) -> Condition<FileRefs> {
        enum Frame<'c> {
            Visit(&'c Clause<KeyRefs>),
            Build(&'c Clause<KeyRefs>),
        }
        let mut frames = vec![Frame::Visit(condition.root())];
        let mut built: Vec<Clause<FileRefs>> = Vec::new();
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

    fn leaf(&self, clause: &Clause<KeyRefs>) -> Clause<FileRefs> {
        let compared = |comparison: &Comparison<KeyRefs>| Comparison {
            decision: self.path(&comparison.decision).clone(),
            value: comparison.value.clone(),
        };
        match clause {
            Clause::Equals(comparison) => Clause::Equals(compared(comparison)),
            Clause::NotEquals(comparison) => Clause::NotEquals(compared(comparison)),
            Clause::Contains(comparison) => Clause::Contains(compared(comparison)),
            Clause::In(membership) => Clause::In(Membership {
                decision: self.path(&membership.decision).clone(),
                values: membership.values.clone(),
            }),
            Clause::Answered(decision) => Clause::Answered(self.path(decision).clone()),
            Clause::All(_) | Clause::Any(_) | Clause::Not(_) => {
                unreachable!("combinations are folded")
            }
        }
    }
}
