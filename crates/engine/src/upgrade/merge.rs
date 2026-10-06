//! The three-way merge behind an upgrade (B7; ARCHITECTURE, Upgrade, save-as-route, re-link):
//! the journey's route version (the base), the version it moves to (the target), and the
//! journey itself, compared by key over nodes (field by field, A1a), edges, participations
//! and resources (by identity), roles, kinds, and the default owner. The journey's local-edit
//! markers (B4) say what it edited on route-copied nodes; roles, kinds, and the default owner
//! carry no markers, so a value that differs from the base's is the journey's edit.
//!
//! Outcomes: a route change the journey did not edit applies; one it edited the same way
//! converges (the marker goes); one it edited differently is a conflict; an edit the route
//! left alone is kept and listed. New route nodes are added unless the journey removed them
//! or their parent (tombstones cover the subtree, B4) or already holds the key; nodes the
//! route removed stay as orphans. A route change that would invalidate the journey's state (a
//! shape change under state or edits, an answered choice removed, a role too narrow for its
//! fill) is a conflict too, and a role or kind the route removed that the journey still uses
//! is kept until the reviewer remaps or removes it. Only clean outcomes are in `merged`;
//! conflicts leave the journey's side in place for their resolutions to change.
//!
//! Cost at the limits (2,000 nodes per graph, 64 edges per node, 32 roles and kinds): three
//! key-indexed graphs already in memory; per node shared by base and journey, 22 field
//! comparisons, its edges (up to 64 each side), its participations (up to 33 kinds) and
//! resources, each O(log n): about 2,000 x (22 + 128 + 66) = 4.3e5 comparisons. Adding nodes
//! builds one tree of the target, O(n log n). Role and kind references scan the merged nodes
//! once per removed role or kind: at most 64 x 2,000. Memory: one clone of the journey graph
//! (at most 16 MiB serialized) plus the lists.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    AnswerSpec, Conflict, Kept, KeyRefs, KindKey, LocalEdit, Node, NodeField, NodeFieldValue,
    NodeKey, NodeState, ParticipationSource, Payload, Provenance, RoleKey, RoleReference,
    removed_choices,
};

use crate::graph::{Document, Tree};

/// What merging the target into a journey gives.
#[derive(Clone, Debug)]
pub(crate) struct Merge {
    /// The journey with every clean outcome applied.
    pub merged: Document,
    /// The conflicts, journey side left in place.
    pub conflicts: Vec<Conflict>,
    /// The journey's edits the route left alone.
    pub kept: Vec<Kept>,
    /// The nodes the route removed, now orphaned.
    pub orphans: Vec<NodeKey>,
}

/// B7: merges `target` into `journey`, which follows `base`.
pub(crate) fn merge(base: &Document, target: &Document, journey: &Document) -> Merge {
    let mut merger = Merger {
        base,
        target,
        journey,
        found: Merge {
            merged: journey.clone(),
            conflicts: Vec::new(),
            kept: Vec::new(),
            orphans: Vec::new(),
        },
    };
    let added = merger.new_nodes();
    let present: BTreeSet<&NodeKey> = journey.nodes.as_map().keys().chain(&added).collect();
    for based in base.nodes.values() {
        match (target.nodes.get(&based.key), journey.nodes.get(&based.key)) {
            (Some(targeted), Some(held)) => merger.node(based, targeted, held, &present),
            (None, Some(_)) => merger.orphan(&based.key),
            (Some(_) | None, None) => {}
        }
    }
    for key in &added {
        merger.add(key, &present);
    }
    merger.default_owner();
    merger.roles();
    merger.kinds();
    assert!(
        merger.found.merged.nodes.len() >= journey.nodes.len(),
        "a merge removes no node"
    );
    merger.found
}

/// A node's kind and, for a decision, its answer type: what no field edit changes.
fn shape(node: &Node<KeyRefs>) -> (cairn_schema::NodeKind, Option<cairn_schema::AnswerType>) {
    let answer = match &node.payload {
        Payload::Decision(decision) => Some(decision.answer.answer_type()),
        Payload::Deliverable(_)
        | Payload::Action(_)
        | Payload::Milestone(_)
        | Payload::Group(_) => None,
    };
    (node.kind(), answer)
}

/// Whether the route changed an aspect a marker names.
fn changed(base: &Node<KeyRefs>, target: &Node<KeyRefs>, edit: &LocalEdit) -> bool {
    match edit {
        LocalEdit::Field(field) => {
            NodeFieldValue::read(*field, base) != NodeFieldValue::read(*field, target)
        }
        LocalEdit::Requires(key) => {
            base.requires.as_set().contains(key) != target.requires.as_set().contains(key)
        }
        LocalEdit::Participation(kind) => {
            base.participations.as_map().get(kind) != target.participations.as_map().get(kind)
        }
        LocalEdit::Resource(key) => resource(base, key) != resource(target, key),
        LocalEdit::Shape => shape(base) != shape(target),
    }
}

fn resource<'a>(
    node: &'a Node<KeyRefs>,
    key: &cairn_schema::AttachmentKey,
) -> Option<&'a cairn_schema::Resource<KeyRefs>> {
    node.resources.iter().find(|found| found.key == *key)
}

struct Merger<'a> {
    base: &'a Document,
    target: &'a Document,
    journey: &'a Document,
    found: Merge,
}

impl Merger<'_> {
    fn markers(&self, key: &NodeKey) -> BTreeSet<LocalEdit> {
        self.journey
            .state
            .local_edits
            .get(key)
            .cloned()
            .unwrap_or_default()
    }

    fn unmark(&mut self, key: &NodeKey, edit: &LocalEdit) {
        let state = &mut self.found.merged.state.local_edits;
        if let Some(edits) = state.get_mut(key) {
            edits.remove(edit);
            if edits.is_empty() {
                state.remove(key);
            }
        }
    }

    fn put(&mut self, node: Node<KeyRefs>) {
        let put = self.found.merged.nodes.put(node);
        assert!(put.is_ok(), "a merge stays within the node limit");
    }

    /// B7, B4: the target's new nodes the journey takes, parents first: not one it holds,
    /// retired, or removed (its tombstone covers the subtree), nor any under one it skips.
    fn new_nodes(&self) -> Vec<NodeKey> {
        let tree = Tree::build(self.target);
        let mut fresh: Vec<&Node<KeyRefs>> = self
            .target
            .nodes
            .values()
            .filter(|node| self.base.nodes.get(&node.key).is_none())
            .collect();
        fresh.sort_by_key(|node| tree.path(&node.key));
        let state = &self.journey.state;
        let mut taken: BTreeSet<&NodeKey> = BTreeSet::new();
        for node in fresh {
            let free = self.journey.nodes.get(&node.key).is_none()
                && !self.journey.retired_keys.nodes.contains(&node.key)
                && !state.tombstones.contains(&node.key);
            let parent_held = node.parent.as_ref().is_none_or(|parent| {
                self.journey.nodes.get(parent).is_some() || taken.contains(parent)
            });
            if free && parent_held {
                taken.insert(&node.key);
            }
        }
        taken.into_iter().cloned().collect()
    }

    /// B1: a new route node, its edges to nodes the journey will not hold dropped, in its
    /// initial state.
    fn add(&mut self, key: &NodeKey, present: &BTreeSet<&NodeKey>) {
        let Some(node) = self.target.nodes.get(key) else {
            unreachable!("a new node is the target's")
        };
        let mut node = with_present_requirements(node, present);
        // A reference to a node the journey removed is left out, and the route's value is a
        // conflict to keep that way or take.
        for field in NodeField::ALL {
            let Some(route) = NodeFieldValue::read(field, &node) else {
                continue;
            };
            if let (true, Some(cleared)) = (dangles(&route, present), cleared(field)) {
                let written = cleared.clone().write(&mut node);
                assert!(written, "the field reads on the node, so it writes");
                self.found.conflicts.push(Conflict::Field {
                    node: key.clone(),
                    journey: cleared,
                    route,
                    dangling: true,
                });
            }
        }
        // A10: so is a message draft whose answer placeholders name such a node.
        let (kept, dangling): (Vec<_>, Vec<_>) = std::mem::take(&mut node.resources)
            .into_iter()
            .partition(|found| !draft_dangles(found, present));
        node.resources = kept;
        for found in dangling {
            self.found.conflicts.push(Conflict::Resource {
                node: key.clone(),
                resource: found.key.clone(),
                journey: None,
                route: Some(found),
                dangling: true,
            });
        }
        let state = NodeState::initial(node.kind(), Provenance::FromRoute);
        self.found.merged.state.nodes.insert(key.clone(), state);
        self.put(node);
    }

    /// B7: a node the route removed stays, orphaned.
    fn orphan(&mut self, key: &NodeKey) {
        if let Some(state) = self.found.merged.state.nodes.get_mut(key) {
            state.provenance = Provenance::Orphaned;
        }
        self.found.orphans.push(key.clone());
    }

    /// B7: one node the base, the target, and the journey share.
    fn node(
        &mut self,
        base: &Node<KeyRefs>,
        target: &Node<KeyRefs>,
        held: &Node<KeyRefs>,
        present: &BTreeSet<&NodeKey>,
    ) {
        let markers = self.markers(&held.key);
        for edit in markers.iter().filter(|edit| !changed(base, target, edit)) {
            self.found.kept.push(Kept::Node {
                node: held.key.clone(),
                edit: edit.clone(),
            });
        }
        if shape(base) != shape(target) {
            self.reshaped(target, held, &markers, present);
            return;
        }
        let mut node = held.clone();
        self.fields(base, target, &mut node, &markers, present);
        self.edges(base, target, &mut node, &markers, present);
        self.participations(base, target, &mut node, &markers);
        self.resources(base, target, &mut node, &markers, present);
        if node != *held {
            self.put(node);
        }
    }

    /// B7: the route changed the node's kind or answer type. A node the journey left as
    /// copied, holding no state, takes the route's whole node (a new kind starting over);
    /// otherwise the whole node is in conflict.
    fn reshaped(
        &mut self,
        target: &Node<KeyRefs>,
        held: &Node<KeyRefs>,
        markers: &BTreeSet<LocalEdit>,
        present: &BTreeSet<&NodeKey>,
    ) {
        let state = self.journey.state.nodes.get(&held.key);
        let answered = self.journey.state.answers.contains_key(&held.key);
        let fresh =
            state.is_none_or(|state| *state == NodeState::initial(held.kind(), state.provenance));
        let whole = !node_dangles(target, present);
        if markers.is_empty() && fresh && !answered && whole {
            if let Some(state) = state.filter(|_| held.kind() != target.kind()) {
                let reset = NodeState::initial(target.kind(), state.provenance);
                self.found
                    .merged
                    .state
                    .nodes
                    .insert(held.key.clone(), reset);
            }
            self.put(with_present_requirements(target, present));
            return;
        }
        self.found.conflicts.push(Conflict::Shape {
            journey: Box::new(held.clone()),
            route: Box::new(target.clone()),
            answered,
            dangling: !whole,
        });
    }

    /// A1a, B7: field by field.
    fn fields(
        &mut self,
        base: &Node<KeyRefs>,
        target: &Node<KeyRefs>,
        node: &mut Node<KeyRefs>,
        markers: &BTreeSet<LocalEdit>,
        present: &BTreeSet<&NodeKey>,
    ) {
        for field in NodeField::ALL {
            let after = NodeFieldValue::read(field, target);
            if NodeFieldValue::read(field, base) == after {
                continue;
            }
            let Some(after) = after else {
                continue;
            };
            let Some(held) = NodeFieldValue::read(field, node) else {
                continue;
            };
            let edit = LocalEdit::Field(field);
            if held == after {
                self.unmark(&node.key, &edit);
            } else if (markers.contains(&edit) && !self.invalidates_answer(&node.key, &after))
                || dangles(&after, present)
            {
                // An edit the route changed too, or a route value naming a node the journey
                // removed: the journey's value stays until the reviewer chooses.
                self.found.conflicts.push(Conflict::Field {
                    node: node.key.clone(),
                    journey: held,
                    dangling: dangles(&after, present),
                    route: after,
                });
            } else if let Some(conflict) = self.answer_conflict(&node.key, &after) {
                self.found.conflicts.push(conflict);
            } else {
                let written = after.write(node);
                assert!(written, "the field reads on the node, so it writes");
            }
        }
    }

    fn invalidates_answer(&self, key: &NodeKey, value: &NodeFieldValue<KeyRefs>) -> bool {
        self.answer_conflict(key, value).is_some()
    }

    /// B7: the route's choices drop a choice the journey's answer names.
    fn answer_conflict(&self, key: &NodeKey, value: &NodeFieldValue<KeyRefs>) -> Option<Conflict> {
        let NodeFieldValue::Choices(choices) = value else {
            return None;
        };
        let answer = self.journey.state.answers.get(key)?;
        (!removed_choices(answer, choices).is_empty()).then(|| Conflict::Answer {
            decision: key.clone(),
            answer: answer.clone(),
            choices: choices.clone(),
        })
    }

    /// A3, B7: explicit edges by the requirement they name; an added one only to a node the
    /// journey will hold.
    fn edges(
        &mut self,
        base: &Node<KeyRefs>,
        target: &Node<KeyRefs>,
        node: &mut Node<KeyRefs>,
        markers: &BTreeSet<LocalEdit>,
        present: &BTreeSet<&NodeKey>,
    ) {
        let (before, after) = (base.requires.as_set(), target.requires.as_set());
        for requirement in before.symmetric_difference(after) {
            let wanted = after.contains(requirement);
            let held = node.requires.as_set().contains(requirement);
            let edit = LocalEdit::Requires(requirement.clone());
            if held == wanted {
                self.unmark(&node.key, &edit);
            } else if markers.contains(&edit) {
                self.found.conflicts.push(Conflict::Edge {
                    edge: cairn_schema::Edge {
                        node: node.key.clone(),
                        requires: requirement.clone(),
                    },
                    journey: held,
                    route: wanted,
                });
            } else if !wanted {
                crate::edit::remove_requirement(node, requirement);
            } else if present.contains(requirement) {
                let added = crate::edit::add_requirement(node, requirement);
                if !added {
                    // Past the edge limit with the journey's own edges: the journey keeps its
                    // edges, and the route's shows as a conflict to resolve.
                    self.found.conflicts.push(Conflict::Edge {
                        edge: cairn_schema::Edge {
                            node: node.key.clone(),
                            requires: requirement.clone(),
                        },
                        journey: false,
                        route: true,
                    });
                }
            }
        }
    }

    /// E2, B7: participations by kind.
    fn participations(
        &mut self,
        base: &Node<KeyRefs>,
        target: &Node<KeyRefs>,
        node: &mut Node<KeyRefs>,
        markers: &BTreeSet<LocalEdit>,
    ) {
        let (before, after) = (base.participations.as_map(), target.participations.as_map());
        let kinds: BTreeSet<&KindKey> = before.keys().chain(after.keys()).collect();
        for kind in kinds {
            let wanted = after.get(kind);
            if before.get(kind) == wanted {
                continue;
            }
            let held = node.participations.as_map().get(kind).cloned();
            let edit = LocalEdit::Participation(kind.clone());
            if held.as_ref() == wanted {
                self.unmark(&node.key, &edit);
            } else if markers.contains(&edit) {
                self.found.conflicts.push(Conflict::Participation {
                    node: node.key.clone(),
                    kind: kind.clone(),
                    journey: held,
                    route: wanted.cloned(),
                });
            } else {
                let set = crate::edit::set_participation(node, kind, wanted.cloned());
                assert!(set, "the route's kinds fit the kind limit");
            }
        }
    }

    /// A10, B7: resources by key.
    fn resources(
        &mut self,
        base: &Node<KeyRefs>,
        target: &Node<KeyRefs>,
        node: &mut Node<KeyRefs>,
        markers: &BTreeSet<LocalEdit>,
        present: &BTreeSet<&NodeKey>,
    ) {
        let keys: BTreeSet<&cairn_schema::AttachmentKey> = base
            .resources
            .iter()
            .chain(&target.resources)
            .map(|found| &found.key)
            .collect();
        for key in keys {
            let wanted = resource(target, key);
            if resource(base, key) == wanted {
                continue;
            }
            let held = resource(node, key).cloned();
            let edit = LocalEdit::Resource(key.clone());
            if held.as_ref() == wanted {
                self.unmark(&node.key, &edit);
            } else if markers.contains(&edit)
                || wanted.is_some_and(|wanted| draft_dangles(wanted, present))
            {
                self.found.conflicts.push(Conflict::Resource {
                    node: node.key.clone(),
                    resource: key.clone(),
                    journey: held,
                    dangling: wanted.is_some_and(|wanted| draft_dangles(wanted, present)),
                    route: wanted.cloned(),
                });
            } else if let Some(wanted) = wanted {
                crate::edit::put_resource(node, wanted.clone());
            } else {
                node.resources.retain(|found| found.key != *key);
            }
        }
    }

    /// A6, B7: the graph's default owner, by value.
    fn default_owner(&mut self) {
        let (before, after) = (&self.base.default_owner, &self.target.default_owner);
        let held = &self.journey.default_owner;
        if before == after {
            if held != before {
                self.found.kept.push(Kept::DefaultOwner);
            }
        } else if held == before {
            self.found.merged.default_owner.clone_from(after);
        } else if held != after {
            self.found.conflicts.push(Conflict::DefaultOwner {
                journey: held.clone(),
                route: after.clone(),
            });
        }
    }

    /// A6, B7: roles by value; a removed one goes only when unedited and unused.
    fn roles(&mut self) {
        let keys: BTreeSet<&RoleKey> = self
            .base
            .roles
            .as_map()
            .keys()
            .chain(self.target.roles.as_map().keys())
            .collect();
        for key in keys {
            let (before, after) = (self.base.roles.get(key), self.target.roles.get(key));
            let held = self.journey.roles.get(key);
            let references = || role_references(&self.found.merged, key);
            let conflict = match (before, after, held) {
                (None, Some(after), None) => {
                    let put = self.found.merged.roles.put(after.clone());
                    assert!(put.is_ok(), "the route's roles fit the role limit");
                    None
                }
                (Some(before), Some(after), Some(held)) if before == after => {
                    if held != before {
                        self.found.kept.push(Kept::Role(key.clone()));
                    }
                    None
                }
                (Some(before), Some(after), Some(held)) if held == before => {
                    let references = references();
                    let fill_too_large = !after.multi
                        && references.iter().any(|reference| {
                            matches!(reference, RoleReference::Fill { entities } if entities.len() > 1)
                        });
                    if fill_too_large {
                        Some((Some(held), Some(after), references))
                    } else {
                        let put = self.found.merged.roles.put(after.clone());
                        assert!(put.is_ok(), "replacing a role keeps the count");
                        None
                    }
                }
                (Some(_), Some(after), Some(held)) if held != after => {
                    Some((Some(held), Some(after), references()))
                }
                (Some(before), None, Some(held)) => {
                    let references = references();
                    if held == before && references.is_empty() {
                        self.found.merged.roles.remove(key);
                        self.found.merged.retired_keys.roles.insert(key.clone());
                        None
                    } else {
                        Some((Some(held), None, references))
                    }
                }
                _ => None,
            };
            if let Some((journey, route, references)) = conflict {
                self.found.conflicts.push(Conflict::Role {
                    role: key.clone(),
                    journey: journey.cloned(),
                    route: route.cloned(),
                    references,
                });
            }
        }
    }

    /// A7, B7: participation kinds by value; a removed one goes only when unedited and
    /// unused.
    fn kinds(&mut self) {
        let base = self.base.participation_kinds.as_map();
        let target = self.target.participation_kinds.as_map();
        let keys: BTreeSet<&KindKey> = base.keys().chain(target.keys()).collect();
        for key in keys {
            let (before, after) = (base.get(key), target.get(key));
            let held = self.journey.participation_kinds.get(key);
            let conflict = match (before, after, held) {
                (None, Some(after), None) => {
                    let put = self.found.merged.participation_kinds.put(after.clone());
                    assert!(put.is_ok(), "the route's kinds fit the kind limit");
                    false
                }
                (Some(before), Some(after), Some(held)) if before == after => {
                    if held != before {
                        self.found.kept.push(Kept::Kind(key.clone()));
                    }
                    false
                }
                (Some(before), Some(after), Some(held)) if held == before => {
                    let put = self.found.merged.participation_kinds.put(after.clone());
                    assert!(put.is_ok(), "replacing a kind keeps the count");
                    false
                }
                (Some(_), Some(after), Some(held)) => held != after,
                (Some(before), None, Some(held)) => {
                    let unused = kind_references(&self.found.merged, key).is_empty();
                    if held == before && unused {
                        self.found.merged.participation_kinds.remove(key);
                        self.found.merged.retired_keys.kinds.insert(key.clone());
                    }
                    held != before || !unused
                }
                _ => false,
            };
            if conflict {
                self.found.conflicts.push(Conflict::Kind {
                    kind: key.clone(),
                    journey: held.cloned(),
                    route: after.cloned(),
                    references: kind_references(&self.found.merged, key),
                });
            }
        }
    }
}

/// The nodes a field value names: a parent, a condition's decisions, a date rule's sources,
/// a stage bound, a fed milestone.
fn named(value: &NodeFieldValue<KeyRefs>) -> Vec<&NodeKey> {
    match value {
        NodeFieldValue::Parent(Some(key))
        | NodeFieldValue::OpensAt(Some(key))
        | NodeFieldValue::ClosesAt(Some(key))
        | NodeFieldValue::FeedsMilestone(Some(key)) => vec![key],
        NodeFieldValue::RelevantWhen(Some(condition)) => condition.decisions(),
        NodeFieldValue::DueBy(Some(found)) | NodeFieldValue::NotBefore(Some(found)) => {
            sources(found)
        }
        _ => Vec::new(),
    }
}

/// A date rule's node sources.
fn sources(rule: &cairn_schema::DateRule<KeyRefs>) -> Vec<&NodeKey> {
    rule.sources
        .as_set()
        .iter()
        .filter_map(|source| match source {
            cairn_schema::DateSource::Node(key) => Some(key),
            cairn_schema::DateSource::CreatedAt => None,
        })
        .collect()
}

/// B4, B7: whether a route value names a node the journey will not hold (one it removed, or
/// a new one it skips), like an edge to an absent node.
fn dangles(value: &NodeFieldValue<KeyRefs>, present: &BTreeSet<&NodeKey>) -> bool {
    named(value).into_iter().any(|key| !present.contains(key))
}

/// A10, B4: whether a resource is a message draft whose answer placeholders name a node the
/// journey will not hold.
fn draft_dangles(found: &cairn_schema::Resource<KeyRefs>, present: &BTreeSet<&NodeKey>) -> bool {
    match &found.content {
        cairn_schema::ResourceContent::MessageDraft(template) => {
            template.segments().iter().any(|segment| {
                matches!(segment, cairn_schema::Segment::Answer(key) if !present.contains(key))
            })
        }
        _ => false,
    }
}

/// Whether any field or message draft of a route node names a node the journey will not hold.
fn node_dangles(node: &Node<KeyRefs>, present: &BTreeSet<&NodeKey>) -> bool {
    let fields = NodeField::ALL.into_iter().any(|field| {
        NodeFieldValue::read(field, node).is_some_and(|value| dangles(&value, present))
    });
    fields
        || node
            .resources
            .iter()
            .any(|found| draft_dangles(found, present))
}

/// A field without its reference, for one that may be absent.
fn cleared(field: NodeField) -> Option<NodeFieldValue<KeyRefs>> {
    Some(match field {
        NodeField::RelevantWhen => NodeFieldValue::RelevantWhen(None),
        NodeField::DueBy => NodeFieldValue::DueBy(None),
        NodeField::NotBefore => NodeFieldValue::NotBefore(None),
        NodeField::OpensAt => NodeFieldValue::OpensAt(None),
        NodeField::ClosesAt => NodeFieldValue::ClosesAt(None),
        NodeField::FeedsMilestone => NodeFieldValue::FeedsMilestone(None),
        NodeField::Parent => NodeFieldValue::Parent(None),
        _ => return None,
    })
}

/// The node with its explicit edges kept only to nodes the journey will hold.
fn with_present_requirements(node: &Node<KeyRefs>, present: &BTreeSet<&NodeKey>) -> Node<KeyRefs> {
    let mut node = node.clone();
    let gone: Vec<NodeKey> = node
        .requires
        .iter()
        .filter(|key| !present.contains(key))
        .cloned()
        .collect();
    for key in &gone {
        crate::edit::remove_requirement(&mut node, key);
    }
    node
}

/// A6, A10, E3: everything in a graph that refers to a role, message drafts included.
pub(crate) fn role_references(graph: &Document, role: &RoleKey) -> BTreeSet<RoleReference> {
    let mut found = BTreeSet::new();
    for node in graph.nodes.values() {
        for (kind, source) in node.participations.as_map() {
            if *source == ParticipationSource::Role(role.clone()) {
                found.insert(RoleReference::Participation {
                    node: node.key.clone(),
                    kind: kind.clone(),
                });
            }
        }
        if let Payload::Decision(decision) = &node.payload
            && let AnswerSpec::Entity { fills_role } | AnswerSpec::EntityList { fills_role } =
                &decision.answer
            && fills_role.as_ref() == Some(role)
        {
            found.insert(RoleReference::FillsRole {
                node: node.key.clone(),
            });
        }
        for resource in &node.resources {
            if let cairn_schema::ResourceContent::MessageDraft(template) = &resource.content
                && template.segments().iter().any(
                    |segment| matches!(segment, cairn_schema::Segment::RoleName(named) if named == role),
                )
            {
                found.insert(RoleReference::Draft {
                    node: node.key.clone(),
                    resource: resource.clone(),
                });
            }
        }
    }
    if let Some(entities) = graph.state.role_fills.get(role) {
        found.insert(RoleReference::Fill {
            entities: entities.clone(),
        });
    }
    if graph.default_owner.as_ref() == Some(role) {
        found.insert(RoleReference::DefaultOwner);
    }
    found
}

/// A7: every node's participation of a kind.
pub(crate) fn kind_references(
    graph: &Document,
    kind: &KindKey,
) -> BTreeMap<NodeKey, ParticipationSource<KeyRefs>> {
    graph
        .nodes
        .values()
        .filter_map(|node| {
            node.participations
                .as_map()
                .get(kind)
                .map(|source| (node.key.clone(), source.clone()))
        })
        .collect()
}
