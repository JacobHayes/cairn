//! Property-test strategies for the engine (PRACTICES, Property tests): generated journeys
//! within the limits and generated operation sequences, with generic content only. An
//! [`Op`] carries indices, resolved against the records as they are when it applies, so a
//! generated sequence stays meaningful as the graph changes (nodes added and removed).
#![allow(clippy::missing_panics_doc)]

pub mod generated;

use std::collections::BTreeSet;

use proptest::prelude::*;

use cairn_schema::{
    Actor, JourneyId, Mutation, Mutations, NodeKey, NodeKind, Patch, PatchId, PatchTarget,
    Transition, from_yaml,
};

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

/// Derive inputs at the fixed clock, over `deployment`, with the PRD's rank constants.
#[must_use]
pub fn derive_inputs(deployment: cairn_schema::Deployment) -> cairn_schema::DeriveInputs {
    cairn_schema::DeriveInputs {
        today: parse("2026-10-06"),
        timezone: parse("UTC"),
        rank: cairn_schema::RankConstants::default(),
        viewer: BTreeSet::new(),
        deployment,
    }
}

/// Runs `patch`'s mutations over `records` as [`apply`] does, without the validation that
/// follows, and returns the operations its node removals spent, the removal index's
/// building included (A18): the removal cost test budgets it.
///
/// # Panics
///
/// When a mutation is rejected.
#[must_use]
pub fn removal_operation_count(records: &Records, patch: &Patch) -> u64 {
    let inputs = fixed_inputs();
    let mut session = crate::mutate::Session::new(patch, &inputs, records.clone());
    for (ordinal, mutation) in patch.mutations.as_slice().iter().enumerate() {
        session.ordinal = u32::try_from(ordinal).unwrap_or(u32::MAX);
        crate::mutate::apply(&mut session, mutation, &mut Vec::new());
    }
    assert!(session.violations.is_empty(), "{:#?}", session.violations);
    session.removal_operations
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
    /// Move node `node` under node `parent`.
    Move {
        /// The node, by index.
        node: u16,
        /// The new parent, by index.
        parent: u16,
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

/// The most operations one generated patch holds.
pub const PATCH_OP_COUNT_MAX: usize = 6;

/// The operations of one generated patch: 1 to [`PATCH_OP_COUNT_MAX`], applied together, so
/// a later mutation meets what an earlier one in the same patch did (J2, A17).
pub fn arb_patch() -> impl Strategy<Value = Vec<Op>> {
    prop::collection::vec(arb_ops(), 1..=PATCH_OP_COUNT_MAX)
}

/// The operations of one generated structural patch: 2 to 12 node additions, edges, notes,
/// moves, and removals, removal-heavy, so a removal often meets what earlier mutations in its patch
/// added, moved under, or linked to the node it removes (A18).
pub fn arb_structure_patch() -> impl Strategy<Value = Vec<Op>> {
    let kind = prop_oneof![
        Just(NodeKind::Action),
        Just(NodeKind::Deliverable),
        Just(NodeKind::Group),
    ];
    let op = prop_oneof![
        3 => (any::<u16>(), kind).prop_map(|(parent, kind)| Op::AddChild { parent, kind }),
        2 => (any::<u16>(), any::<u16>()).prop_map(|(node, requires)| Op::AddEdge { node, requires }),
        1 => any::<u16>().prop_map(|node| Op::Artifact { node }),
        1 => (any::<u16>(), any::<u16>()).prop_map(|(node, parent)| Op::Move { node, parent }),
        3 => any::<u16>().prop_map(|node| Op::Remove { node }),
    ];
    prop::collection::vec(op, 2..=12)
}

/// The one patch `ops` make against the records as they are now. Each operation resolves
/// against the records the ones before it leave, found by applying each alone to a scratch
/// copy; one that copy rejects leaves it as it was, so the operations after it resolve as
/// if it were not there (the patch holding it is then most likely rejected, as generated
/// single operations often are).
///
/// # Panics
///
/// When `ops` is empty or longer than a patch may be.
#[must_use]
pub fn resolve_patch(ops: &[Op], records: &Records) -> Patch {
    resolve_in_sequence(ops, records).0
}

/// [`resolve_patch`], with the records the operations leave when each, applied alone in
/// turn, is accepted; none when one is rejected.
///
/// # Panics
///
/// When `ops` is empty or longer than a patch may be.
#[must_use]
pub fn resolve_in_sequence(ops: &[Op], records: &Records) -> (Patch, Option<Records>) {
    assert!(!ops.is_empty(), "a patch holds at least one mutation");
    let mut scratch = records.clone();
    let mut every_accepted = true;
    let mut mutations = Vec::with_capacity(ops.len());
    for op in ops {
        let mutation = op.mutation(&scratch);
        let alone = patch(&scratch, vec![mutation.clone()]);
        match apply(&scratch, &alone, &fixed_inputs()) {
            Ok(applied) => scratch = applied.records().clone(),
            Err(_) => every_accepted = false,
        }
        mutations.push(mutation);
    }
    (patch(records, mutations), every_accepted.then_some(scratch))
}

impl Op {
    /// The patch this operation makes against the records as they are now.
    #[must_use]
    pub fn resolve(&self, records: &Records) -> Patch {
        patch(records, vec![self.mutation(records)])
    }

    /// The mutation this operation makes against the records as they are now.
    fn mutation(&self, records: &Records) -> Mutation {
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
        match (self, pick(0)) {
            (Op::AddChild { parent, kind }, _) => add_child(graph, pick(*parent), *kind, fresh),
            (
                Op::AddEdge { .. }
                | Op::Answer { .. }
                | Op::Transition { .. }
                | Op::Pin { .. }
                | Op::Snooze { .. }
                | Op::Artifact { .. }
                | Op::Move { .. }
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
            (Op::Move { node, parent }, Some(_)) => yaml(&format!(
                "op: set_node_field\nnode: {}\nvalue: {{parent: {}}}\n",
                key_text(pick(*node)),
                key_text(pick(*parent))
            )),
            (Op::Remove { node }, Some(key)) => removal(graph, pick(*node).unwrap_or(key)),
        }
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
    Mutation::RemoveNode {
        removal: crate::edit::full_removal(graph, node),
    }
}
