//! Property-test strategies for the engine (PRACTICES, Property tests): generated journeys
//! within the limits and generated operation sequences, with generic content only. An
//! [`Op`] carries indices, resolved against the records as they are when it applies, so a
//! generated sequence stays meaningful as the graph changes (nodes added and removed).
#![allow(clippy::missing_panics_doc)]

use std::collections::BTreeSet;

use proptest::prelude::*;

use cairn_schema::{
    Actor, Edge, JourneyId, Mutation, Mutations, NodeKey, NodeKind, ParticipationRef, Patch,
    PatchId, PatchTarget, Removal, Transition, from_yaml,
};

use crate::graph::Tree;
use crate::pipeline::{ApplyInputs, apply};
use crate::records::Records;
use crate::transition::Move;

/// The journey generated graphs build.
#[must_use]
pub fn journey_id() -> JourneyId {
    parse("j_generated")
}

fn parse<T: std::str::FromStr>(text: &str) -> T
where
    T::Err: std::fmt::Debug,
{
    match text.parse() {
        Ok(value) => value,
        Err(error) => panic!("{text:?} does not parse: {error:?}"),
    }
}

/// The clock and actor generated operations apply with.
#[must_use]
pub fn fixed_inputs() -> ApplyInputs {
    ApplyInputs {
        today: parse("2026-10-06"),
        at: parse("2026-10-06T12:00:00Z"),
        actor: Actor {
            user: parse("u_generated"),
            agent: None,
        },
        note: None,
    }
}

/// One generated operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// Add a node of `kind` under node `parent` (a root when the graph is empty or the parent
    /// cannot hold children).
    AddChild {
        /// The parent, by index.
        parent: u16,
        /// The kind.
        kind: NodeKind,
    },
    /// Make node `node` require node `requires`.
    AddEdge {
        /// The dependent, by index.
        node: u16,
        /// The requirement, by index.
        requires: u16,
    },
    /// Answer decision `node` (or whatever node the index lands on) yes or no.
    Answer {
        /// The node, by index.
        node: u16,
        /// The answer.
        yes: bool,
    },
    /// Move node `node` by `step`.
    Transition {
        /// The node, by index.
        node: u16,
        /// The transition.
        step: Move,
    },
    /// Pin node `node` `day` days into October 2026.
    Pin {
        /// The node, by index.
        node: u16,
        /// The day of the month, from 0.
        day: u8,
    },
    /// Snooze node `node` until node `until`.
    Snooze {
        /// The node, by index.
        node: u16,
        /// The node it waits on, by index.
        until: u16,
    },
    /// Link an artifact to node `node`.
    Artifact {
        /// The node, by index.
        node: u16,
    },
    /// Remove node `node`, naming everything its removal reaches.
    Remove {
        /// The node, by index.
        node: u16,
    },
}

/// One generated operation.
pub fn arb_ops() -> impl Strategy<Value = Op> {
    let kind = prop_oneof![
        Just(NodeKind::Decision),
        Just(NodeKind::Deliverable),
        Just(NodeKind::Action),
        Just(NodeKind::Milestone),
        Just(NodeKind::Group),
    ];
    let step = prop::sample::select(Move::ALL.to_vec());
    prop_oneof![
        3 => (any::<u16>(), kind).prop_map(|(parent, kind)| Op::AddChild { parent, kind }),
        2 => (any::<u16>(), any::<u16>()).prop_map(|(node, requires)| Op::AddEdge { node, requires }),
        2 => (any::<u16>(), any::<bool>()).prop_map(|(node, yes)| Op::Answer { node, yes }),
        3 => (any::<u16>(), step).prop_map(|(node, step)| Op::Transition { node, step }),
        1 => (any::<u16>(), 0..30_u8).prop_map(|(node, day)| Op::Pin { node, day }),
        1 => (any::<u16>(), any::<u16>()).prop_map(|(node, until)| Op::Snooze { node, until }),
        1 => any::<u16>().prop_map(|node| Op::Artifact { node }),
        1 => any::<u16>().prop_map(|node| Op::Remove { node }),
    ]
}

/// A generated journey: created empty, then grown by up to 48 generated additions, each
/// accepted by apply (so every generated graph holds every invariant).
pub fn arb_graph() -> impl Strategy<Value = Records> {
    let additions = prop::collection::vec(arb_ops(), 0..48);
    additions.prop_map(|ops| {
        let mut records = Records::default();
        let created = patch(&records, vec![create()]);
        records = match apply(&records, &created, &fixed_inputs()) {
            Ok(applied) => applied.records().clone(),
            Err(rejection) => panic!("an empty journey is always created: {rejection:#?}"),
        };
        for op in ops
            .into_iter()
            .filter(|op| matches!(op, Op::AddChild { .. } | Op::AddEdge { .. }))
        {
            if let Ok(applied) = apply(&records, &op.resolve(&records), &fixed_inputs()) {
                records = applied.records().clone();
            }
        }
        records
    })
}

fn create() -> Mutation {
    from_yaml("op: create_journey\nname: Generated\n").unwrap_or_else(|error| panic!("{error}"))
}

fn patch(records: &Records, mutations: Vec<Mutation>) -> Patch {
    let journey = journey_id();
    Patch {
        id: parse::<PatchId>("p_generated"),
        base_revision: records.revision(&cairn_schema::Domain::Journey(journey.clone())),
        target: PatchTarget::Journey(journey),
        deployment_revision: None,
        mutations: Mutations::new(mutations).unwrap_or_else(|error| panic!("{error}")),
    }
}

impl Op {
    /// The patch this operation makes against the records as they are now.
    #[must_use]
    pub fn resolve(&self, records: &Records) -> Patch {
        let empty = cairn_schema::Graph::default();
        let graph = records
            .journeys
            .get(&journey_id())
            .map_or(&empty, |journey| &journey.graph);
        let keys: Vec<&NodeKey> = graph.nodes.as_map().keys().collect();
        let pick = |index: u16| -> Option<&NodeKey> {
            let count = keys.len();
            (count > 0)
                .then(|| keys.get(usize::from(index) % count).copied())
                .flatten()
        };
        let fresh = graph.nodes.len() + graph.retired_keys.nodes.len() + 1;
        let mutation = match (self, pick(0)) {
            (Op::AddChild { parent, kind }, _) => add_child(graph, pick(*parent), *kind, fresh),
            (
                Op::AddEdge { .. }
                | Op::Answer { .. }
                | Op::Transition { .. }
                | Op::Pin { .. }
                | Op::Snooze { .. }
                | Op::Artifact { .. }
                | Op::Remove { .. },
                None,
            ) => add_child(graph, None, NodeKind::Action, fresh),
            (Op::AddEdge { node, requires }, Some(_)) => yaml(&format!(
                "op: add_edge\nedge: {{node: {}, requires: {}}}\n",
                key_text(pick(*node)),
                key_text(pick(*requires))
            )),
            (Op::Answer { node, yes }, Some(_)) => yaml(&format!(
                "op: answer\ndecision: {}\nvalue: {{boolean: {yes}}}\n",
                key_text(pick(*node))
            )),
            (Op::Transition { node, step }, Some(_)) => transition(key_text(pick(*node)), *step),
            (Op::Pin { node, day }, Some(_)) => yaml(&format!(
                "op: set_pin\nnode: {}\ndate: \"2026-10-{:02}\"\n",
                key_text(pick(*node)),
                day % 28 + 1
            )),
            (Op::Snooze { node, until }, Some(_)) => yaml(&format!(
                "op: snooze\nnode: {}\nuntil: {{node: {}}}\n",
                key_text(pick(*node)),
                key_text(pick(*until))
            )),
            (Op::Artifact { node }, Some(_)) => yaml(&format!(
                "op: add_annotation\nannotation: {{key: a_g{fresh}, node: {}, artifact: \"https://example.org/{fresh}\"}}\n",
                key_text(pick(*node))
            )),
            (Op::Remove { node }, Some(key)) => removal(graph, pick(*node).unwrap_or(key)),
        };
        patch(records, vec![mutation])
    }
}

fn yaml(text: &str) -> Mutation {
    from_yaml(text).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn key_text(key: Option<&NodeKey>) -> &str {
    key.map_or("n_missing", NodeKey::as_str)
}

fn add_child(
    graph: &cairn_schema::Graph,
    parent: Option<&NodeKey>,
    kind: NodeKind,
    fresh: usize,
) -> Mutation {
    let parent = parent
        .filter(|parent| {
            graph
                .nodes
                .get(parent)
                .is_some_and(|node| !node.kind().is_leaf())
        })
        .map(|parent| format!("\n  parent: {parent}"))
        .unwrap_or_default();
    let decision = match kind {
        NodeKind::Decision => "\n  prompt: Yes?\n  answer_type: boolean",
        NodeKind::Deliverable | NodeKind::Action | NodeKind::Milestone | NodeKind::Group => "",
    };
    yaml(&format!(
        "op: add_node\nnode:\n  key: n_g{fresh}\n  id: g{fresh}\n  kind: {}\n  title: Generated {fresh}{parent}{decision}\n",
        kind.name()
    ))
}

fn transition(node: &str, step: Move) -> Mutation {
    let transition = match step {
        Move::Start => Transition::Start,
        Move::Stop => Transition::Stop,
        Move::Complete => Transition::Complete,
        Move::Skip => Transition::Skip {
            reason: parse("Generated."),
        },
        Move::Reopen => Transition::Reopen,
        Move::Reach => Transition::Reach,
        Move::Answer => {
            return yaml(&format!(
                "op: answer\ndecision: {node}\nvalue: {{boolean: true}}\n"
            ));
        }
    };
    Mutation::Transition {
        node: parse(node),
        transition,
    }
}

/// A removal naming everything the node's removal reaches (A18), so it is accepted unless
/// something else forbids it.
fn removal(graph: &cairn_schema::Graph, node: &NodeKey) -> Mutation {
    let tree = Tree::build(graph);
    let descendants: BTreeSet<NodeKey> = tree.descendants(node).into_iter().collect();
    let inside = |key: &NodeKey| key == node || descendants.contains(key);
    let mut removal = Removal {
        node: node.clone(),
        descendants: descendants.clone(),
        edges: BTreeSet::new(),
        resources: BTreeSet::new(),
        annotations: BTreeSet::new(),
        participations: BTreeSet::new(),
    };
    for dependent in graph.nodes.values() {
        for requirement in dependent.requires.iter() {
            if inside(&dependent.key) || inside(requirement) {
                removal.edges.insert(Edge {
                    node: dependent.key.clone(),
                    requires: requirement.clone(),
                });
            }
        }
        if inside(&dependent.key) {
            removal.resources.extend(
                dependent
                    .resources
                    .iter()
                    .map(|resource| resource.key.clone()),
            );
            removal
                .participations
                .extend(
                    dependent
                        .participations
                        .as_map()
                        .keys()
                        .map(|kind| ParticipationRef {
                            node: dependent.key.clone(),
                            kind: kind.clone(),
                        }),
                );
        }
    }
    let notes = graph
        .state
        .annotations
        .values()
        .filter(|note| note.body.node.as_ref().is_some_and(inside));
    removal
        .annotations
        .extend(notes.map(|note| note.body.key.clone()));
    Mutation::RemoveNode { removal }
}
