//! Building a graph from a file document (A13, A14; PRD, Identity and references): supplied
//! keys are kept; against a base version, a keyless node takes the key of the base node at
//! its path, a keyless role or kind the key of the base one with its id, and a keyless
//! resource the key of the base node's resource with its title; the rest are minted through
//! the host's allocator, never as a key the base holds or retired. Every path and id is
//! resolved to a key, and the result is validated like any other graph, so every fixture
//! loads through the model.
//!
//! Cost at the limits: one pass over the file's nodes, roles, and kinds to collect keys and
//! paths (and one over the base's to match them), and one to resolve references by map
//! lookup, O(n log n), then graph validation.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    AnswerSpec, AttachmentKey, Clause, Comparison, Condition, DateRule, DateSource, Decision,
    FileRefs, Group, KeyAllocator, KeyRefs, Keyed, KindKey, Membership, MessageTemplate, Node,
    NodeField, NodeKey, OneOrMany, ParticipationKind, ParticipationSource, Participations, Path,
    Payload, Prefixed, Resource, ResourceContent, Role, RoleKey, RouteFile, Segment, Slug, Subject,
    Violation, ViolationCode, Violations, mint,
};

use crate::graph::{Document, Graph};
use crate::validate::violation;

/// Builds a graph from a route file with no base version: keys kept or minted, references
/// resolved, invariants checked (A13, A15).
///
/// # Errors
///
/// Every key repeated, path that does not resolve, and invariant the graph breaks.
///
/// # Panics
///
/// When the allocator keeps returning keys already in use, or the engine's own key
/// accounting breaks.
pub fn from_file(file: &RouteFile, allocator: &mut dyn KeyAllocator) -> Result<Graph, Violations> {
    import(file, None, allocator)
}

/// A13: builds a graph from a route file against the version it extends: an unchanged node
/// keeps its key by path, a moved node keeps the key it carries, an unknown path mints a new
/// one.
///
/// # Errors
///
/// Every key repeated, path that does not resolve, and invariant the graph breaks.
///
/// # Panics
///
/// When the allocator keeps returning keys already in use, or the engine's own key
/// accounting breaks.
pub fn import(
    file: &RouteFile,
    base: Option<&Document>,
    allocator: &mut dyn KeyAllocator,
) -> Result<Graph, Violations> {
    let matched;
    let (file, reserved) = match base {
        Some(base) => {
            matched = super::matching::with_base_keys(file, base);
            (&matched, super::matching::Reserved::of(base))
        }
        None => (file, super::matching::Reserved::default()),
    };
    let mut keys = Keys::collect(file, allocator, &reserved);
    let mut resolver = Resolver::new(file, &keys);
    let roles = file
        .roles
        .as_slice()
        .iter()
        .zip(keys.roles.drain(..))
        .map(|(role, key)| Role {
            key,
            id: role.id.clone(),
            title: role.title.clone(),
            multi: role.multi,
        });
    let kinds = file
        .participation_kinds
        .as_slice()
        .iter()
        .zip(keys.kinds.drain(..))
        .map(|(kind, key)| ParticipationKind {
            key,
            id: kind.id.clone(),
            title: kind.title.clone(),
            multi: kind.multi,
        });
    let (roles, kinds): (Vec<_>, Vec<_>) = (roles.collect(), kinds.collect());
    let default_owner = file
        .default_owner
        .as_ref()
        .and_then(|role| resolver.role(role, None));
    let mut resource_keys = std::mem::take(&mut keys.resources).into_iter();
    let nodes: Vec<Node<KeyRefs>> = file
        .nodes
        .as_slice()
        .iter()
        .zip(&keys.nodes)
        .map(|(node, key)| resolver.node(node, key, &mut resource_keys))
        .collect();
    assert!(
        resource_keys.next().is_none(),
        "every resource key was used"
    );
    let mut violations = keys.violations;
    if !violations.is_empty() {
        // Two objects with one key cannot be put in one graph to check it further.
        violations.append(&mut resolver.violations);
        return Err(collected(violations));
    }
    let document = Document {
        default_owner,
        roles: keyed(roles),
        participation_kinds: keyed(kinds),
        nodes: keyed(nodes),
        ..Document::default()
    };
    // A15: references that did not resolve were left out, so every invariant the rest of the
    // graph breaks is reported with them.
    violations.append(&mut resolver.violations);
    // A route has no answers, so no deployment changes its relevance.
    Graph::checked(document, violations, &cairn_schema::Deployment::default())
}

fn collected(violations: Vec<Violation>) -> Violations {
    match Violations::new(violations) {
        Ok(violations) => violations,
        Err(error) => unreachable!("collected only when there is one: {error}"),
    }
}

/// Collects values whose keys were checked unique and whose count the file already bounded.
fn keyed<V: cairn_schema::HasKey, L: cairn_schema::collections::LimitOf>(
    values: Vec<V>,
) -> Keyed<V, L> {
    match Keyed::new(values) {
        Ok(keyed) => keyed,
        Err(error) => panic!("keys were checked unique and counts bounded by the file: {error}"),
    }
}

/// Every object's key, supplied or minted, in file order, and the repeats found.
struct Keys {
    nodes: Vec<NodeKey>,
    roles: Vec<RoleKey>,
    kinds: Vec<KindKey>,
    resources: Vec<AttachmentKey>,
    violations: Vec<Violation>,
}

impl Keys {
    fn collect(
        file: &RouteFile,
        allocator: &mut dyn KeyAllocator,
        reserved: &super::matching::Reserved,
    ) -> Self {
        let nodes = file.nodes.as_slice();
        let resources: Vec<Option<AttachmentKey>> = nodes
            .iter()
            .flat_map(|node| node.resources.iter().map(|resource| resource.key.clone()))
            .collect();
        let mut violations = Vec::new();
        let supplied_nodes: Vec<_> = nodes.iter().map(|node| node.key.clone()).collect();
        let roles: Vec<_> = file
            .roles
            .as_slice()
            .iter()
            .map(|role| role.key.clone())
            .collect();
        let kinds: Vec<_> = file
            .participation_kinds
            .as_slice()
            .iter()
            .map(|kind| kind.key.clone())
            .collect();
        let retired = &reserved.retired;
        refuse_retired(
            &supplied_nodes,
            &retired.nodes,
            Subject::Node,
            &mut violations,
        );
        refuse_retired(&roles, &retired.roles, Subject::Role, &mut violations);
        refuse_retired(&kinds, &retired.kinds, Subject::Kind, &mut violations);
        let keys = Keys {
            nodes: fill(
                supplied_nodes,
                allocator,
                &mut violations,
                Subject::Node,
                &reserved.nodes,
            ),
            roles: fill(
                roles,
                allocator,
                &mut violations,
                Subject::Role,
                &reserved.roles,
            ),
            kinds: fill(
                kinds,
                allocator,
                &mut violations,
                Subject::Kind,
                &reserved.kinds,
            ),
            resources: fill(
                resources,
                allocator,
                &mut violations,
                Subject::Attachment,
                &reserved.resources,
            ),
            violations,
        };
        assert_eq!(keys.nodes.len(), nodes.len());
        assert_eq!(keys.roles.len(), file.roles.len());
        keys
    }
}

/// Invariants (no key is ever reused): a supplied key the base retired is rejected.
fn refuse_retired<K: Ord + Clone + std::fmt::Display>(
    supplied: &[Option<K>],
    retired: &BTreeSet<K>,
    subject: fn(K) -> Subject,
    violations: &mut Vec<Violation>,
) {
    for key in supplied
        .iter()
        .flatten()
        .filter(|key| retired.contains(*key))
    {
        let mut found = violation(
            ViolationCode::RetiredKeyReused,
            format!("the key {key} was retired from the route and cannot come back"),
        );
        found.at.subject = Some(subject(key.clone()));
        violations.push(found);
    }
}

/// Keeps each supplied key, reporting repeats, and mints the missing ones, skipping any body
/// the allocator returns that is already in use or `reserved`.
fn fill<K: Prefixed + Ord + Clone>(
    supplied: Vec<Option<K>>,
    allocator: &mut dyn KeyAllocator,
    violations: &mut Vec<Violation>,
    subject: fn(K) -> Subject,
    reserved: &BTreeSet<K>,
) -> Vec<K> {
    let mut used: BTreeSet<K> = BTreeSet::new();
    for key in supplied.iter().flatten() {
        if !used.insert(key.clone()) {
            let mut found = violation(
                ViolationCode::DuplicateKey,
                format!("the key {key} is given to two objects"),
            );
            found.at.subject = Some(subject(key.clone()));
            violations.push(found);
        }
    }
    let attempts_max = (supplied.len() + reserved.len()) * 2 + 1;
    let mut keys = Vec::with_capacity(supplied.len());
    for key in supplied {
        let key = key.unwrap_or_else(|| {
            for _ in 0..attempts_max {
                let candidate: K = mint(allocator);
                if !reserved.contains(&candidate) && used.insert(candidate.clone()) {
                    return candidate;
                }
            }
            panic!("the key allocator kept returning keys already in use")
        });
        keys.push(key);
    }
    keys
}

/// Resolves paths and ids to keys, collecting every reference that does not resolve.
struct Resolver {
    paths: BTreeMap<Path, NodeKey>,
    roles: BTreeMap<Slug, RoleKey>,
    kinds: BTreeMap<Slug, KindKey>,
    violations: Vec<Violation>,
    /// The node being resolved, for locations.
    at: Option<Path>,
}

impl Resolver {
    fn new(file: &RouteFile, keys: &Keys) -> Self {
        let mut resolver = Resolver {
            paths: BTreeMap::new(),
            roles: BTreeMap::new(),
            kinds: BTreeMap::from([(cairn_schema::id::owner_kind_id(), KindKey::owner())]),
            violations: Vec::new(),
            at: None,
        };
        // A path past the depth limit or a sibling id given twice is the graph check's to
        // report (from the tree); here the first holder of a path keeps it.
        for (node, key) in file.nodes.as_slice().iter().zip(&keys.nodes) {
            let path = match &node.parent {
                None => Ok(Path::root(node.id.clone())),
                Some(parent) => parent.child(node.id.clone()),
            };
            if let Ok(path) = path {
                resolver.paths.entry(path).or_insert_with(|| key.clone());
            }
        }
        for (role, key) in file.roles.as_slice().iter().zip(&keys.roles) {
            resolver.roles.insert(role.id.clone(), key.clone());
        }
        let kinds = file.participation_kinds.as_slice().iter().zip(&keys.kinds);
        for (kind, key) in kinds {
            resolver.kinds.insert(kind.id.clone(), key.clone());
        }
        resolver
    }

    fn unresolved(&mut self, what: String, field: Option<NodeField>, code: ViolationCode) {
        let mut found = violation(code, what);
        found.at.path.clone_from(&self.at);
        found.at.field = field;
        self.violations.push(found);
    }

    fn path(&mut self, path: &Path, field: Option<NodeField>) -> Option<NodeKey> {
        let key = self.paths.get(path).cloned();
        if key.is_none() {
            let what = format!("no node has the path {path}");
            self.unresolved(what, field, ViolationCode::UnresolvedReference);
        }
        key
    }

    fn role(&mut self, id: &Slug, field: Option<NodeField>) -> Option<RoleKey> {
        let key = self.roles.get(id).cloned();
        if key.is_none() {
            let what = format!("no role has the id {id}");
            self.unresolved(what, field, ViolationCode::UnresolvedReference);
        }
        key
    }

    fn kind(&mut self, id: &Slug) -> Option<KindKey> {
        let key = self.kinds.get(id).cloned();
        if key.is_none() {
            let what = format!("no participation kind has the id {id}");
            self.unresolved(what, None, ViolationCode::UndeclaredKind);
        }
        key
    }

    /// Resolves one node. A reference that does not resolve is reported and left out (a
    /// parent that does not resolve leaves the node a root), so the rest of the graph can
    /// still be checked (A15).
    fn node(
        &mut self,
        node: &Node<FileRefs>,
        key: &NodeKey,
        resource_keys: &mut std::vec::IntoIter<AttachmentKey>,
    ) -> Node<KeyRefs> {
        self.at = Some(match &node.parent {
            None => Path::root(node.id.clone()),
            Some(parent) => parent
                .child(node.id.clone())
                .unwrap_or_else(|_| parent.clone()),
        });
        let parent = node
            .parent
            .as_ref()
            .and_then(|parent| self.path(parent, Some(NodeField::Parent)));
        let requires: Vec<NodeKey> = node
            .requires
            .iter()
            .filter_map(|path| self.path(path, None))
            .collect();
        let relevant_when = node
            .relevant_when
            .as_ref()
            .and_then(|condition| self.condition(condition));
        let due_by = node
            .due_by
            .as_ref()
            .and_then(|rule| self.rule(rule, NodeField::DueBy));
        let not_before = node
            .not_before
            .as_ref()
            .and_then(|rule| self.rule(rule, NodeField::NotBefore));
        let participations = self.participations(&node.participations);
        let resources = self.resources(&node.resources, resource_keys);
        let payload = self.payload(&node.payload);
        assert!(requires.len() <= node.requires.len() && resources.len() <= node.resources.len());
        Node {
            key: key.clone(),
            id: node.id.clone(),
            parent,
            title: node.title.clone(),
            description: node.description.clone(),
            weight: node.weight,
            requires: cairn_schema::BoundedSet::new(requires).unwrap_or_default(),
            relevant_when,
            due_by,
            not_before,
            participations: participations.unwrap_or_default(),
            resources,
            payload,
        }
    }

    /// Resolves a condition tree bottom up with an explicit stack (PRACTICES, No recursion).
    fn condition(&mut self, condition: &Condition<FileRefs>) -> Option<Condition<KeyRefs>> {
        enum Frame<'a> {
            Visit(&'a Clause<FileRefs>),
            Build(&'a Clause<FileRefs>),
        }
        let field = Some(NodeField::RelevantWhen);
        let mut frames = vec![Frame::Visit(condition.root())];
        let mut built: Vec<Option<Clause<KeyRefs>>> = Vec::new();
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
                    Clause::Equals(_)
                    | Clause::NotEquals(_)
                    | Clause::In(_)
                    | Clause::Contains(_)
                    | Clause::Answered(_) => {
                        let resolved = self.leaf(clause, field);
                        built.push(resolved);
                    }
                },
                Frame::Build(clause) => {
                    let folded = fold(clause, &mut built);
                    built.push(folded);
                }
            }
        }
        let root = built.pop().flatten()?;
        assert!(built.is_empty(), "every clause was folded into the root");
        match Condition::new(root) {
            Ok(condition) => Some(condition),
            Err(error) => panic!("a resolved condition keeps its shape and limits: {error}"),
        }
    }
}

/// Folds a combination's resolved children, the last ones built, into it.
fn fold(
    clause: &Clause<FileRefs>,
    built: &mut Vec<Option<Clause<KeyRefs>>>,
) -> Option<Clause<KeyRefs>> {
    let count = match clause {
        Clause::All(children) | Clause::Any(children) => children.len(),
        Clause::Not(_) => 1,
        Clause::Equals(_)
        | Clause::NotEquals(_)
        | Clause::In(_)
        | Clause::Contains(_)
        | Clause::Answered(_) => unreachable!("only combinations are folded"),
    };
    assert!(
        built.len() >= count,
        "a combination's children were built first"
    );
    let children = built.split_off(built.len() - count);
    let mut children: Vec<Clause<KeyRefs>> = children.into_iter().collect::<Option<_>>()?;
    match clause {
        Clause::All(_) => Some(Clause::All(children)),
        Clause::Any(_) => Some(Clause::Any(children)),
        Clause::Not(_) => children.pop().map(|child| Clause::Not(Box::new(child))),
        Clause::Equals(_)
        | Clause::NotEquals(_)
        | Clause::In(_)
        | Clause::Contains(_)
        | Clause::Answered(_) => unreachable!("only combinations are folded"),
    }
}

impl Resolver {
    fn leaf(
        &mut self,
        clause: &Clause<FileRefs>,
        field: Option<NodeField>,
    ) -> Option<Clause<KeyRefs>> {
        let comparison = |resolver: &mut Self, comparison: &Comparison<FileRefs>| {
            resolver
                .path(&comparison.decision, field)
                .map(|decision| Comparison {
                    decision,
                    value: comparison.value.clone(),
                })
        };
        match clause {
            Clause::Equals(compared) => comparison(self, compared).map(Clause::Equals),
            Clause::NotEquals(compared) => comparison(self, compared).map(Clause::NotEquals),
            Clause::Contains(compared) => comparison(self, compared).map(Clause::Contains),
            Clause::In(membership) => self.path(&membership.decision, field).map(|decision| {
                Clause::In(Membership {
                    decision,
                    values: membership.values.clone(),
                })
            }),
            Clause::Answered(decision) => self.path(decision, field).map(Clause::Answered),
            Clause::All(_) | Clause::Any(_) | Clause::Not(_) => {
                unreachable!("combinations are folded, not resolved as leaves")
            }
        }
    }

    fn rule(&mut self, rule: &DateRule<FileRefs>, field: NodeField) -> Option<DateRule<KeyRefs>> {
        let mut sources = Vec::new();
        for source in rule.sources.as_set() {
            match source {
                DateSource::CreatedAt => sources.push(DateSource::CreatedAt),
                DateSource::Node(path) => {
                    sources.extend(self.path(path, Some(field)).map(DateSource::Node));
                }
            }
        }
        let sources = OneOrMany::new(sources).ok()?;
        Some(DateRule {
            direction: rule.direction,
            sources,
            offset: rule.offset,
        })
    }

    fn participations(
        &mut self,
        participations: &Participations<FileRefs>,
    ) -> Option<Participations<KeyRefs>> {
        let mut map = BTreeMap::new();
        for (kind, source) in participations.as_map() {
            let kind = self.kind(kind);
            let source = match source {
                ParticipationSource::Role(role) => {
                    self.role(role, None).map(ParticipationSource::Role)
                }
                ParticipationSource::Entities(entities) => {
                    Some(ParticipationSource::Entities(entities.clone()))
                }
            };
            if let (Some(kind), Some(source)) = (kind, source) {
                map.insert(kind, source);
            }
        }
        Participations::try_from(map).ok()
    }

    fn resources(
        &mut self,
        resources: &[Resource<FileRefs>],
        keys: &mut std::vec::IntoIter<AttachmentKey>,
    ) -> Vec<Resource<KeyRefs>> {
        let mut resolved = Vec::with_capacity(resources.len());
        for resource in resources {
            let Some(key) = keys.next() else {
                unreachable!("a key was collected for every resource")
            };
            let content = match &resource.content {
                ResourceContent::Tip(text) => Some(ResourceContent::Tip(text.clone())),
                ResourceContent::Template(url) => Some(ResourceContent::Template(url.clone())),
                ResourceContent::Example(url) => Some(ResourceContent::Example(url.clone())),
                ResourceContent::Reference(url) => Some(ResourceContent::Reference(url.clone())),
                ResourceContent::MessageDraft(template) => {
                    self.template(template).map(ResourceContent::MessageDraft)
                }
            };
            if let Some(content) = content {
                resolved.push(Resource {
                    key,
                    title: resource.title.clone(),
                    content,
                });
            }
        }
        resolved
    }

    /// Resolves a message draft's placeholders, writing it in the graph form and reading it
    /// back, so the template grammar stays the schema's.
    fn template(
        &mut self,
        template: &MessageTemplate<FileRefs>,
    ) -> Option<MessageTemplate<KeyRefs>> {
        let mut text = String::new();
        let mut complete = true;
        // Every placeholder is resolved, so each one that does not resolve is reported (A15).
        for segment in template.segments() {
            let resolved: Option<Segment<KeyRefs>> = match segment {
                Segment::Text(literal) => Some(Segment::Text(literal.clone())),
                Segment::Journey(field) => Some(Segment::Journey(*field)),
                Segment::RoleName(role) => self.role(role, None).map(Segment::RoleName),
                Segment::Answer(path) => self.path(path, None).map(Segment::Answer),
            };
            match resolved {
                Some(resolved) => {
                    text.push_str(&MessageTemplate::<KeyRefs>::render_segment(&resolved));
                }
                None => complete = false,
            }
        }
        if !complete {
            return None;
        }
        // Keys can be longer than the paths and ids they replace, so a draft within the body
        // limit as written can pass it resolved.
        if cairn_schema::Limit::BodyBytes.check(text.len()).is_err() {
            let what = format!(
                "the message draft resolves to {} bytes, past body_bytes_max",
                text.len()
            );
            self.unresolved(what, None, ViolationCode::LimitExceeded);
            if let Some(found) = self.violations.last_mut() {
                found.limit = Some(cairn_schema::Limit::BodyBytes);
            }
            return None;
        }
        match text.parse() {
            Ok(template) => Some(template),
            Err(error) => panic!("a resolved message draft within its limit reads back: {error}"),
        }
    }

    fn payload(&mut self, payload: &Payload<FileRefs>) -> Payload<KeyRefs> {
        match payload {
            Payload::Decision(decision) => Payload::Decision(Decision {
                prompt: decision.prompt.clone(),
                help: decision.help.clone(),
                answer: self.answer_spec(&decision.answer),
            }),
            Payload::Deliverable(deliverable) => Payload::Deliverable(deliverable.clone()),
            Payload::Action(action) => Payload::Action(action.clone()),
            Payload::Milestone(milestone) => Payload::Milestone(milestone.clone()),
            Payload::Group(group) => Payload::Group(Group {
                opens_at: self.optional_path(group.opens_at.as_ref(), NodeField::OpensAt),
                closes_at: self.optional_path(group.closes_at.as_ref(), NodeField::ClosesAt),
                gates: group.gates,
                closes: group.closes,
            }),
        }
    }

    /// The key a reference names; none when there is no reference or it does not resolve
    /// (reported).
    fn optional_path(&mut self, path: Option<&Path>, field: NodeField) -> Option<NodeKey> {
        path.and_then(|path| self.path(path, Some(field)))
    }

    fn answer_spec(&mut self, spec: &AnswerSpec<FileRefs>) -> AnswerSpec<KeyRefs> {
        let fills = |resolver: &mut Self, role: Option<&Slug>| {
            role.and_then(|role| resolver.role(role, Some(NodeField::FillsRole)))
        };
        match spec {
            AnswerSpec::Boolean => AnswerSpec::Boolean,
            AnswerSpec::SingleChoice(choices) => AnswerSpec::SingleChoice(choices.clone()),
            AnswerSpec::MultiChoice(choices) => AnswerSpec::MultiChoice(choices.clone()),
            AnswerSpec::Text => AnswerSpec::Text,
            AnswerSpec::Date { feeds_milestone } => AnswerSpec::Date {
                feeds_milestone: self
                    .optional_path(feeds_milestone.as_ref(), NodeField::FeedsMilestone),
            },
            AnswerSpec::Entity { fills_role } => AnswerSpec::Entity {
                fills_role: fills(self, fills_role.as_ref()),
            },
            AnswerSpec::EntityList { fills_role } => AnswerSpec::EntityList {
                fills_role: fills(self, fills_role.as_ref()),
            },
        }
    }
}
