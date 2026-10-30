//! What removing a node, a role or a participation kind leaves dangling, and the mutations
//! that rewrite it (A18). One translator, [`Dropping`], is the whole rule: it is
//! [`Node::map_references`] with the removed things answering none, so a new reference field
//! cannot compile without being dropped here. The plan compares each remaining node with its
//! translation and sends the difference; the removal read lists the references the
//! translation dropped.
//!
//! Cost: one pass over the nodes outside the removal, each translated once, plus the
//! removal's own index (O(n log n + edges)).

use std::collections::BTreeSet;

use cairn_schema::{
    AttachmentKey, Dangling, DanglingKind, Graph, KeyRefs, KindColumns, KindKey, MapReferences,
    MessageTemplate, Mutation, Node, NodeField, NodeFieldValue, NodeKey, OutsideReference,
    ReferenceField, RemovalPlan, RemovalTarget, Resource, ResourceContent, RoleKey, Segment,
    SnoozeTarget,
};

use crate::edit::full_removals;

/// What is removed: the nodes inside a removal, a role, or a kind.
#[derive(Default)]
struct Dropping {
    nodes: BTreeSet<NodeKey>,
    role: Option<RoleKey>,
    kind: Option<KindKey>,
    /// The fields of the translated node that named a removed node, with the node.
    hits: Vec<(NodeField, NodeKey)>,
}

impl MapReferences<KeyRefs, KeyRefs> for Dropping {
    fn node(&mut self, node: &NodeKey, field: Option<NodeField>) -> Option<NodeKey> {
        if !self.nodes.contains(node) {
            return Some(node.clone());
        }
        // A requirement and a message draft's placeholder have no field; the removal
        // already names the first, and the plan reads the second from the draft itself.
        self.hits.extend(field.map(|field| (field, node.clone())));
        None
    }

    fn role(&mut self, role: &RoleKey, _: Option<NodeField>) -> Option<RoleKey> {
        (self.role.as_ref() != Some(role)).then(|| role.clone())
    }

    fn kind(&mut self, kind: &KindKey) -> Option<KindKey> {
        (self.kind.as_ref() != Some(kind)).then(|| kind.clone())
    }

    fn own_key(&mut self, key: &NodeKey) -> NodeKey {
        key.clone()
    }

    fn resource_key(&mut self, key: &AttachmentKey) -> AttachmentKey {
        key.clone()
    }

    fn draft_too_long(&mut self, _: &AttachmentKey, bytes: usize) {
        unreachable!("dropping shortens a draft, which is {bytes} bytes")
    }

    fn kinds_collapsed(&mut self, kind: &KindKey) {
        unreachable!("dropping maps each kind to itself: {kind}")
    }
}

/// A message draft after its placeholders that name something gone are dropped.
pub(crate) enum Pruned {
    /// It names nothing gone.
    Unchanged,
    /// Nothing else was left: it is no draft.
    Emptied,
    /// The draft without them.
    To(MessageTemplate<KeyRefs>),
}

/// The draft without the placeholders `gone` says are, keeping its other text.
pub(crate) fn pruned_draft(
    template: &MessageTemplate<KeyRefs>,
    gone: impl Fn(&Segment<KeyRefs>) -> bool,
) -> Pruned {
    if !template.segments().iter().any(&gone) {
        return Pruned::Unchanged;
    }
    let text: String = template
        .segments()
        .iter()
        .filter(|segment| !gone(segment))
        .map(MessageTemplate::<KeyRefs>::render_segment)
        .collect();
    text.parse().map_or(Pruned::Emptied, Pruned::To)
}

/// The edit of a message draft to leave out the placeholders `dropping` removes, or its
/// removal when nothing else is left; none when it names none of them.
fn draft_without(
    node: &NodeKey,
    resource: &Resource<KeyRefs>,
    template: &MessageTemplate<KeyRefs>,
    dropping: &Dropping,
) -> Option<Mutation> {
    let gone = |segment: &Segment<KeyRefs>| match segment {
        Segment::Answer(node) => dropping.nodes.contains(node),
        Segment::RoleName(role) => dropping.role.as_ref() == Some(role),
        Segment::Text(_) | Segment::Journey(_) => false,
    };
    match pruned_draft(template, gone) {
        Pruned::Unchanged => None,
        Pruned::To(template) => {
            let mut next = resource.clone();
            next.content = ResourceContent::MessageDraft(template);
            Some(Mutation::EditResource {
                node: node.clone(),
                resource: next,
            })
        }
        Pruned::Emptied => Some(Mutation::RemoveResource {
            node: node.clone(),
            resource: resource.key.clone(),
        }),
    }
}

fn field(node: &NodeKey, value: NodeFieldValue<KeyRefs>) -> Mutation {
    Mutation::SetNodeField {
        node: node.clone(),
        value,
    }
}

/// The fixes for the references of `node` that `dropping` removes.
fn dangling_on(node: &Node<KeyRefs>, dropping: &mut Dropping, out: &mut Vec<Dangling>) {
    let kept = node.map_references(dropping);
    let before = KindColumns::of(&node.payload);
    let after = KindColumns::of(&kept.payload);
    let mut push = |what, mutation| {
        out.push(Dangling {
            node: Some(node.key.clone()),
            what,
            mutation,
        });
    };
    // The clauses that survive stay; the walker alone would drop the whole condition.
    let relevant_when = node.relevant_when.as_ref().and_then(|condition| {
        condition.without_decisions(|decision| dropping.nodes.contains(decision))
    });
    if relevant_when != node.relevant_when {
        let value = NodeFieldValue::RelevantWhen(relevant_when);
        push(DanglingKind::RelevantWhen, field(&node.key, value));
    }
    if kept.due_by != node.due_by {
        let value = NodeFieldValue::DueBy(kept.due_by);
        push(DanglingKind::DueBy, field(&node.key, value));
    }
    if kept.not_before != node.not_before {
        let value = NodeFieldValue::NotBefore(kept.not_before);
        push(DanglingKind::NotBefore, field(&node.key, value));
    }
    if after.feeds_milestone != before.feeds_milestone {
        let value = NodeFieldValue::FeedsMilestone(after.feeds_milestone);
        push(DanglingKind::FeedsMilestone, field(&node.key, value));
    }
    if after.fills_role != before.fills_role {
        let value = NodeFieldValue::FillsRole(after.fills_role);
        push(DanglingKind::FillsRole, field(&node.key, value));
    }
    for kind in node.participations.as_map().keys() {
        if !kept.participations.as_map().contains_key(kind) {
            let mutation = Mutation::ClearParticipation {
                node: node.key.clone(),
                kind: kind.clone(),
            };
            push(DanglingKind::Participation, mutation);
        }
    }
    for resource in &node.resources {
        let ResourceContent::MessageDraft(template) = &resource.content else {
            continue;
        };
        if let Some(mutation) = draft_without(&node.key, resource, template, dropping) {
            push(DanglingKind::MessageDraft, mutation);
        }
    }
}

/// A18: what removing `target` from `graph` takes and leaves dangling, with the mutations
/// that rewrite the dangling references, to send before the removal. None when a removed
/// node is not in the graph.
#[must_use]
pub fn plan_removal(graph: &Graph, target: &RemovalTarget) -> Option<RemovalPlan> {
    let mut dropping = Dropping::default();
    let mut plan = RemovalPlan {
        removal: None,
        nodes: Vec::new(),
        dangling: Vec::new(),
    };
    match target {
        RemovalTarget::Node(key) => {
            graph.nodes.get(key)?;
            let removal = full_removals(graph, std::slice::from_ref(key)).pop()?;
            dropping.nodes = removal.nodes().cloned().collect();
            plan.nodes = removal.nodes().cloned().collect();
            plan.removal = Some(removal);
        }
        RemovalTarget::Role(role) => dropping.role = Some(role.clone()),
        RemovalTarget::Kind(kind) => dropping.kind = Some(kind.clone()),
    }
    let inside = dropping.nodes.clone();
    for node in graph
        .nodes
        .values()
        .filter(|node| !inside.contains(&node.key))
    {
        dangling_on(node, &mut dropping, &mut plan.dangling);
    }
    for (holder, until) in &graph.state.snoozes {
        if matches!(until, SnoozeTarget::Node(on) if dropping.nodes.contains(on))
            && !dropping.nodes.contains(holder)
        {
            plan.dangling.push(Dangling {
                node: Some(holder.clone()),
                what: DanglingKind::Snooze,
                mutation: Mutation::Unsnooze {
                    node: holder.clone(),
                },
            });
        }
    }
    if graph.default_owner.is_some() && graph.default_owner == dropping.role {
        plan.dangling.push(Dangling {
            node: None,
            what: DanglingKind::DefaultOwner,
            mutation: Mutation::SetDefaultOwner { role: None },
        });
    }
    Some(plan)
}

/// A18: the references from outside the nodes `inside` that name one of them.
pub(crate) fn outside_references(
    graph: &Graph,
    inside: &BTreeSet<&NodeKey>,
) -> BTreeSet<OutsideReference> {
    let mut dropping = Dropping {
        nodes: inside.iter().map(|key| (*key).clone()).collect(),
        ..Dropping::default()
    };
    let mut found = BTreeSet::new();
    for node in graph
        .nodes
        .values()
        .filter(|node| !inside.contains(&node.key))
    {
        dropping.hits.clear();
        let _ = node.map_references(&mut dropping);
        let named = dropping
            .hits
            .iter()
            .filter_map(|(field, target)| Some((ReferenceField::of(*field)?, target.clone())));
        found.extend(named.map(|(field, target)| OutsideReference {
            holder: node.key.clone(),
            field,
            target,
        }));
        for resource in &node.resources {
            if let ResourceContent::MessageDraft(template) = &resource.content {
                for segment in template.segments() {
                    if let Segment::Answer(target) = segment
                        && inside.contains(target)
                    {
                        found.insert(OutsideReference {
                            holder: node.key.clone(),
                            field: ReferenceField::MessageDraft,
                            target: target.clone(),
                        });
                    }
                }
            }
        }
    }
    for (holder, until) in &graph.state.snoozes {
        if let SnoozeTarget::Node(target) = until
            && !inside.contains(holder)
            && inside.contains(target)
        {
            found.insert(OutsideReference {
                holder: holder.clone(),
                field: ReferenceField::Snooze,
                target: target.clone(),
            });
        }
    }
    found
}
