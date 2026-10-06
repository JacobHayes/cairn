//! The effective dependency graph (PRD Containment, Gating, F4; Invariants: the cycle check
//! over implicit edges): inherited requirements through the entry chain with the ancestor
//! named, force include dropping condition gates but not requirements, gate and date-only
//! edges, a container completing only at its own finish, and cycles judged on gate edges.
#![cfg(test)]

mod support;

use std::collections::BTreeSet;

use cairn_engine::derive::dependencies::{EdgeClass, EdgeSource, Instant, Point};
use cairn_engine::derive::{Dependencies, EdgeSet, EffectiveDependency};
use cairn_schema::{DependencyVia, NodeKey, ViolationCode};
use support::key;

fn dependency(node: &str, via: DependencyVia) -> EffectiveDependency {
    EffectiveDependency {
        node: key(node),
        via,
        class: EdgeClass::Gate,
    }
}

/// Every node whose finish `from` reaches through edges of the set and classes given.
fn reached(
    graph: &Dependencies,
    from: Instant,
    set: EdgeSet,
    gates_only: bool,
) -> BTreeSet<NodeKey> {
    let mut seen = BTreeSet::from([from]);
    let mut stack = vec![from];
    let mut found = BTreeSet::new();
    while let Some(instant) = stack.pop() {
        for edge in graph.waits(instant, set) {
            if gates_only && edge.class != EdgeClass::Gate {
                continue;
            }
            if edge.requirement.point == Point::Finish {
                found.insert(graph.key(edge.requirement.node).unwrap().clone());
            }
            if seen.insert(edge.requirement) {
                stack.push(edge.requirement);
            }
        }
    }
    found
}

fn start_of(graph: &Dependencies, node: &str) -> Instant {
    Instant::new(graph.node_index(&key(node)).unwrap(), Point::Start)
}

/// Containment: the test plan requires environment access, so its child actions wait for
/// it too, through the plan's entry, and the explanation names the plan.
#[test]
fn a_childs_actions_wait_for_their_parents_requirement() {
    let derived = support::derived(&support::vendor_after(1), "j_vendor_eval");
    let graph = derived.dependencies();
    for child in ["n_plan_draft", "n_plan_review"] {
        let listed = graph.of(&key(child), EdgeSet::Pruned);
        let inherited = dependency(
            "n_access",
            DependencyVia::Inherited {
                ancestor: key("n_plan"),
            },
        );
        assert!(listed.contains(&inherited), "{child}: {listed:#?}");
        let waits_on = reached(graph, start_of(graph, child), EdgeSet::Pruned, true);
        assert!(
            waits_on.contains(&key("n_access")),
            "{child} waits for access"
        );
    }
    assert!(
        graph
            .of(&key("n_plan"), EdgeSet::Pruned)
            .contains(&dependency("n_access", DependencyVia::Explicit))
    );
}

/// Containment: a parent requires its children, and a child with an estimate fits between
/// its parent's entry and its parent's start: entry, child start, child finish, parent
/// start, parent finish, with no cycle.
#[test]
fn a_child_fits_between_its_parents_entry_and_start() {
    let derived = support::derived(&support::vendor_after(1), "j_vendor_eval");
    let graph = derived.dependencies();
    let plan = graph.node_index(&key("n_plan")).unwrap();
    let draft = graph.node_index(&key("n_plan_draft")).unwrap();
    let chain = [
        (
            Instant::new(draft, Point::Start),
            Instant::new(plan, Point::Entry),
        ),
        (
            Instant::new(draft, Point::Finish),
            Instant::new(draft, Point::Start),
        ),
        (
            Instant::new(plan, Point::Start),
            Instant::new(draft, Point::Finish),
        ),
        (
            Instant::new(plan, Point::Finish),
            Instant::new(plan, Point::Start),
        ),
    ];
    for (later, earlier) in chain {
        assert!(
            graph
                .waits(later, EdgeSet::Full)
                .any(|edge| edge.requirement == earlier),
            "{later:?} waits for {earlier:?}"
        );
    }
    assert!(
        graph
            .of(&key("n_plan"), EdgeSet::Pruned)
            .contains(&dependency("n_plan_draft", DependencyVia::Containment))
    );
    let ahead = reached(
        graph,
        Instant::new(plan, Point::Entry),
        EdgeSet::Full,
        false,
    );
    assert!(
        !ahead.contains(&key("n_plan_draft")),
        "no cycle through the entry"
    );
}

/// A container is complete only at its own finish: every dependent waits for a finish,
/// and a container's finish waits only on its own start, which waits on its children.
#[test]
fn reaching_a_containers_entry_or_start_never_completes_it() {
    let derived = support::derived(&support::vendor_after(1), "j_vendor_eval");
    let graph = derived.dependencies();
    let testing = graph.node_index(&key("n_testing")).unwrap();
    let dependents: Vec<_> = graph
        .edges(EdgeSet::Full)
        .filter(|edge| edge.requirement.node == testing && edge.dependent.node != testing)
        // Children wait on its entries: that is the chain, not a dependency on it.
        .filter(|edge| edge.source != EdgeSource::Chain)
        .collect();
    assert!(!dependents.is_empty(), "reporting requires testing");
    assert!(
        dependents
            .iter()
            .all(|edge| edge.requirement.point == Point::Finish)
    );
    let finish: Vec<_> = graph
        .waits(Instant::new(testing, Point::Finish), EdgeSet::Full)
        .map(|edge| (edge.requirement, edge.source))
        .collect();
    assert_eq!(
        finish,
        [(Instant::new(testing, Point::Start), EdgeSource::Work)]
    );
    let start_sources: BTreeSet<_> = graph
        .waits(Instant::new(testing, Point::Start), EdgeSet::Full)
        .map(|edge| edge.source)
        .collect();
    assert_eq!(
        start_sources,
        BTreeSet::from([EdgeSource::Entry, EdgeSource::Containment])
    );
}

const GATED_BOX: &str = "- op: add_node\n  node: {key: n_first, id: first, kind: decision, title: First, prompt: First?, answer_type: boolean}\n\
- op: add_node\n  node: {key: n_prior, id: prior, kind: action, title: Prior}\n\
- op: add_node\n  node: {key: n_box, id: box, kind: group, title: Box, requires: [n_prior], relevant_when: {equals: {decision: n_first, value: true}}}\n\
- op: add_node\n  node: {key: n_inner, id: inner, parent: n_box, kind: action, title: Inner}\n\
- op: apply_override\n  node: n_inner\n  override: {force_include: {reason: Needed anyway.}}\n";

/// Gating, Force include: a force-included child of a container gated by a condition is not
/// held by that condition but still waits for the container's explicit requirements, even
/// once the container is not relevant.
#[test]
fn force_include_drops_condition_gates_and_keeps_requirements() {
    let condition = dependency(
        "n_first",
        DependencyVia::Condition {
            condition_on: key("n_box"),
        },
    );
    let requirement = dependency(
        "n_prior",
        DependencyVia::Inherited {
            ancestor: key("n_box"),
        },
    );
    let open = support::journey(GATED_BOX);
    let no = "- op: answer\n  decision: n_first\n  value: {boolean: false}\n";
    for (label, records) in [("open", open.clone()), ("no", support::accepted(&open, no))] {
        let derived = support::derived(&records, support::JOURNEY);
        let graph = derived.dependencies();
        let pruned = graph.of(&key("n_inner"), EdgeSet::Pruned);
        assert!(pruned.contains(&requirement), "{label}: {pruned:#?}");
        assert!(!pruned.contains(&condition), "{label}: {pruned:#?}");
        let full = graph.of(&key("n_inner"), EdgeSet::Full);
        assert!(
            full.contains(&condition) && full.contains(&requirement),
            "{label}"
        );
        let waits_on = reached(graph, start_of(graph, "n_inner"), EdgeSet::Pruned, true);
        assert_eq!(waits_on, BTreeSet::from([key("n_prior")]), "{label}");
    }
}

fn stage(gates: bool) -> String {
    format!(
        "- op: add_node\n  node: {{key: n_opens, id: opens, kind: milestone, title: Opens}}\n\
- op: add_node\n  node: {{key: n_stage, id: stage, kind: group, title: Stage, opens_at: n_opens, gates: {gates}}}\n\
- op: add_node\n  node: {{key: n_work, id: work, parent: n_stage, kind: action, title: Work}}\n"
    )
}

/// F4: a stage's contents inherit its opening; with `gates: false` the opening holds dates
/// (date-only) but never blocks.
#[test]
fn a_non_gating_opening_holds_dates_but_never_blocks() {
    for gates in [true, false] {
        let derived = support::derived(&support::journey(&stage(gates)), support::JOURNEY);
        let graph = derived.dependencies();
        let class = if gates {
            EdgeClass::Gate
        } else {
            EdgeClass::DateOnly
        };
        let opening = EffectiveDependency {
            node: key("n_opens"),
            via: DependencyVia::StageOpening {
                group: key("n_stage"),
            },
            class,
        };
        assert!(graph.of(&key("n_work"), EdgeSet::Pruned).contains(&opening));
        let start = start_of(graph, "n_work");
        let blocking = reached(graph, start, EdgeSet::Pruned, true);
        let dating = reached(graph, start, EdgeSet::Pruned, false);
        assert_eq!(blocking.contains(&key("n_opens")), gates, "gates: {gates}");
        assert!(dating.contains(&key("n_opens")), "gates: {gates}");
    }
}

/// Invariants: the acyclicity check reads gate edges only, so a non-gating opening whose
/// milestone requires the group is accepted, and the same with a gating opening is a cycle.
/// Condition gates and stage openings join the check.
#[test]
fn cycles_are_judged_on_gate_edges_with_implicit_ones_included() {
    let requires_stage = "- op: add_edge\n  edge: {node: n_opens, requires: n_stage}\n";
    let date_only = support::journey(&stage(false));
    assert!(support::journey_patch(&date_only, requires_stage).is_ok());
    let gating = support::journey(&stage(true));
    let found = support::codes(support::journey_patch(&gating, requires_stage));
    assert_eq!(found, [ViolationCode::DependencyCycle]);
    // A decision the box's condition reads, requiring work inside the box.
    let records = support::journey(GATED_BOX);
    let condition_cycle = "- op: add_edge\n  edge: {node: n_first, requires: n_inner}\n";
    let found = support::codes(support::journey_patch(&records, condition_cycle));
    assert_eq!(found, [ViolationCode::DependencyCycle]);
}

/// PRACTICES, Back-of-the-envelope: at `node_count_max` nodes, containment at the depth
/// limit, and dozens of explicit edges per node, the graph holds O(nodes + references) edges
/// (each inherited requirement is reached through the chain, never copied), and listing
/// every node's dependencies walks only its entry chains.
#[test]
fn the_graph_stays_linear_at_the_limits() {
    use cairn_engine::testing::generated::{NodeSeed, build};
    use cairn_schema::limits::{CONTAINMENT_DEPTH_MAX, NODE_COUNT_MAX};
    let count = NODE_COUNT_MAX as usize;
    let seeds: Vec<NodeSeed> = (0..count)
        .map(|i| NodeSeed {
            climb: if i % 24 == 23 { 4 } else { 0 },
            kind: [2, 4, 6, 0, 1, 5][i % 6],
            requires: (0..32)
                .map(|j| u16::try_from((i * 131 + j * 977) % 65_536).unwrap())
                .collect(),
            condition: (i % 3 == 0).then(|| (u8::try_from(i % 6).unwrap(), 7, 11)),
            opening: Some((u16::try_from(i).unwrap(), i % 2 == 0)),
            state: u8::try_from(i % 4).unwrap(),
            force: i % 50 == 0,
            keep: false,
            participation: (i % 7 == 0).then(|| (i % 2 == 0, u8::try_from(i % 5).unwrap())),
        })
        .collect();
    let graph = build(&seeds);
    let document = graph.document();
    let depth = document
        .nodes
        .as_map()
        .keys()
        .map(|key| graph.tree().path(key).unwrap().depth())
        .max();
    assert_eq!(depth, Some(CONTAINMENT_DEPTH_MAX));
    let derived = cairn_engine::derive(
        &graph,
        &cairn_engine::testing::derive_inputs(cairn_schema::Deployment::default()),
    );
    let dependencies = derived.dependencies();
    let explicit: usize = document
        .nodes
        .values()
        .map(|node| node.requires.len())
        .sum();
    let edges = dependencies.edges(EdgeSet::Full).count();
    // Six per node for work, entries, and containment; one per reference; conditions here
    // read at most two decisions, and every group has an opening.
    assert!(
        edges <= 6 * count + explicit + 3 * count,
        "{edges} edges, {explicit} explicit"
    );
    assert!(
        explicit > 10 * count,
        "the limit case carries many explicit edges: {explicit}"
    );
    let listed: usize = document
        .nodes
        .as_map()
        .keys()
        .map(|key| dependencies.of(key, EdgeSet::Full).len())
        .sum();
    assert!(
        listed > explicit,
        "inherited requirements are listed on demand: {listed}"
    );
}
