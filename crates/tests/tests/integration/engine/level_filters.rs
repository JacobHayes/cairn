//! C2, C9, C1: a level that filters and the top-level rule, on a hand-built journey. The list's
//! filters and text search keep a node when it matches or holds a match, and the rest hide like
//! a kind left out: they roll up, their edges re-target, and a blocked node waiting on one with
//! no stand-in carries the hidden-prerequisites marker. The top-level rule (Stages) leaves out
//! the subtrees under a top-level node of another kind. The level counts what each class left
//! out hides. The selected node and the containers above it are drawn whatever hides them.
//! Derived at 2026-10-06.
#![cfg(test)]

use crate::engine::support;

use std::collections::{BTreeMap, BTreeSet};

use cairn_engine::{DerivedJourney, Records};
use cairn_schema::{EntityKey, Level, LevelDisplay, LevelQuery, ListFlag, NodeKind};
use support::{add_nodes as add, key};

const PEOPLE: &str = "- op: create_entity\n  entity: {key: e_a, name: A}\n- op: create_entity\n  entity: {key: e_b, name: B}\n";

/// A group holding two deliverables owned by A (one with two actions, which inherit A) and one
/// owned by B, the second action and B's deliverable applying only when a decision says so;
/// and at the top level that decision, a deliverable, and a milestone that requires it.
fn journey() -> Records {
    let nodes = add(&[
        "{key: n_group, id: group, kind: group, title: Group}",
        "{key: n_work, id: work, parent: n_group, kind: deliverable, title: Work, description: Moored at the harbour., participations: {k_owner: [e_a]}}",
        "{key: n_step, id: step, parent: n_work, kind: action, title: Step}",
        "{key: n_other, id: other, parent: n_work, kind: action, title: Other, requires: [n_step], relevant_when: {equals: {decision: n_ask, value: true}}}",
        "{key: n_side, id: side, parent: n_group, kind: deliverable, title: Side, description: Moored too., requires: [n_step, n_other], participations: {k_owner: [e_a]}}",
        "{key: n_spare, id: spare, parent: n_group, kind: deliverable, title: Spare, participations: {k_owner: [e_b]}, relevant_when: {equals: {decision: n_ask, value: true}}}",
        "{key: n_ask, id: ask, kind: decision, title: Ask, prompt: Go?, answer_type: boolean}",
        "{key: n_loose, id: loose, kind: deliverable, title: Loose}",
        "{key: n_gate, id: gate, kind: milestone, title: Gate, requires: [n_loose]}",
    ]);
    let extras = "- op: add_resource\n  node: n_step\n  resource: {key: a_tip, tip: Drop the anchor first.}\n\
- op: add_annotation\n  annotation: {key: a_note, node: n_spare, note: Past the lighthouse.}\n";
    support::journey(&format!("{PEOPLE}{nodes}{extras}"))
}

/// The level of the test journey for `query`, as `viewer` reads it.
fn level_of(records: &Records, query: &LevelQuery, viewer: &[&str]) -> Level {
    let graph = support::journey_graph(records, support::JOURNEY);
    let derived = support::derived(records, support::JOURNEY);
    let viewer: BTreeSet<EntityKey> = viewer.iter().map(|key| key.parse().unwrap()).collect();
    DerivedJourney::new(&graph, &derived)
        .level(query, &records.deployment, &viewer)
        .unwrap()
}

fn every_kind() -> LevelQuery {
    LevelQuery::of_kinds(NodeKind::ALL.into_iter().collect(), None)
}

fn searching(text: &str) -> LevelQuery {
    LevelQuery {
        text: Some(text.parse().unwrap()),
        ..every_kind()
    }
}

/// Each drawn node and the node it is drawn under.
fn placement(level: &Level) -> BTreeMap<&str, Option<&str>> {
    level
        .nodes
        .iter()
        .map(|node| {
            (
                node.key.as_str(),
                node.parent.as_ref().map(cairn_schema::NodeKey::as_str),
            )
        })
        .collect()
}

fn rolled_up<'a>(level: &'a Level, name: &str) -> Vec<&'a str> {
    let node = level.nodes.iter().find(|node| node.key.as_str() == name);
    node.unwrap_or_else(|| panic!("{name} is drawn"))
        .rolled_up
        .iter()
        .map(cairn_schema::NodeKey::as_str)
        .collect()
}

/// C2, C9: text found only in a description, only in a resource, or only in a note draws that
/// node and the containers holding it; what does not match rolls up into the nearest drawn
/// container, and a top-level node holding no match is not drawn.
#[test]
fn a_search_draws_its_matches_and_what_holds_them() {
    let records = journey();
    for (text, expected) in [
        (
            "HARBOUR",
            vec![("n_group", None), ("n_work", Some("n_group"))],
        ),
        (
            "anchor",
            vec![
                ("n_group", None),
                ("n_work", Some("n_group")),
                ("n_step", Some("n_work")),
            ],
        ),
        (
            "lighthouse",
            vec![("n_group", None), ("n_spare", Some("n_group"))],
        ),
    ] {
        let level = level_of(&records, &searching(text), &[]);
        assert_eq!(placement(&level), expected.into_iter().collect(), "{text}");
    }
    let harbour = level_of(&records, &searching("harbour"), &[]);
    assert_eq!(rolled_up(&harbour, "n_work"), ["n_other", "n_step"]);
    assert_eq!(rolled_up(&harbour, "n_group"), ["n_side", "n_spare"]);
}

/// C9, E6: the owner filter follows aliases, and Only mine reads the viewer's own entities.
#[test]
fn the_owner_filter_follows_aliases_and_mine_reads_the_viewer() {
    let records = journey();
    let owned_by = |records: &Records, owner: &str| {
        let query = LevelQuery {
            owner: Some(owner.parse().unwrap()),
            ..every_kind()
        };
        placement(&level_of(records, &query, &[]))
            .into_keys()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let a = ["n_group", "n_other", "n_side", "n_step", "n_work"];
    assert_eq!(
        owned_by(&records, "e_a"),
        a,
        "the actions inherit their deliverable's owner"
    );
    let mine = LevelQuery {
        flags: BTreeSet::from([ListFlag::Mine]),
        ..every_kind()
    };
    let drawn = |viewer: &str| {
        placement(&level_of(&records, &mine, &[viewer]))
            .into_keys()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert_eq!(drawn("e_b"), ["n_group", "n_spare"]);
    assert_eq!(drawn("e_a"), a);
    let revision = records.journeys.values().next().unwrap().revision.get();
    let merge = format!(
        "- op: merge_entities\n  survivor: e_a\n  merged: e_b\n  journeys: {{{}: {revision}}}\n",
        support::JOURNEY
    );
    let patch = support::patch_to(&records, "deployment", &merge);
    let merged = support::apply(&records, &patch, &support::fixed_inputs()).unwrap();
    assert_eq!(
        owned_by(merged.records(), "e_b"),
        [
            "n_group", "n_other", "n_side", "n_spare", "n_step", "n_work"
        ],
        "the merged entity's alias reads as the survivor"
    );
}

/// C2: what the filter leaves out re-targets its edges to its stand-in, two landing on one
/// pair collapse, one landing inside a card is dropped, and a blocked node whose filtered-out
/// prerequisite has no stand-in carries the marker.
#[test]
fn filtered_out_nodes_re_target_their_edges_and_keep_the_marker() {
    let records = journey();
    let level = level_of(&records, &searching("moored"), &[]);
    let edges: Vec<(&str, &str, usize)> = level
        .edges
        .iter()
        .map(|edge| (edge.from.as_str(), edge.to.as_str(), edge.underlying.len()))
        .collect();
    assert_eq!(edges, [("n_work", "n_side", 2)]);
    let gate = level_of(&records, &searching("gate"), &[]);
    assert_eq!(placement(&gate).into_keys().collect::<Vec<_>>(), ["n_gate"]);
    assert_eq!(gate.edges, [] as [cairn_schema::LevelEdge; 0]);
    let marker: Vec<&str> = gate.nodes[0]
        .hidden_prerequisites
        .iter()
        .map(cairn_schema::NodeKey::as_str)
        .collect();
    assert_eq!(marker, ["n_loose"]);
}

/// C2, C4: a match beneath a collapsed container leaves the container drawn, with the match
/// rolled up into it.
#[test]
fn a_match_under_a_collapse_keeps_its_container_drawn() {
    let records = journey();
    let query = LevelQuery {
        collapsed: BTreeSet::from([key("n_group")]),
        ..searching("anchor")
    };
    let level = level_of(&records, &query, &[]);
    assert_eq!(
        placement(&level).into_keys().collect::<Vec<_>>(),
        ["n_group"]
    );
    assert!(rolled_up(&level, "n_group").contains(&"n_step"));
}

/// C2's Stages rule: a top-level node of a kind `top` lacks is left out with its subtree, the
/// edge from it is left off, and the milestone waiting on it is marked. Drilled into a
/// container, the container's top-level ancestor decides.
#[test]
fn the_top_level_rule_leaves_out_other_subtrees() {
    let records = journey();
    let stages = LevelQuery {
        top: BTreeSet::from([NodeKind::Group, NodeKind::Milestone]),
        ..every_kind()
    };
    let level = level_of(&records, &stages, &[]);
    let drawn = placement(&level);
    assert!(!drawn.contains_key("n_loose"));
    assert!(drawn.contains_key("n_gate") && drawn.contains_key("n_step"));
    assert!(
        !level
            .edges
            .iter()
            .any(|edge| edge.from.as_str() == "n_loose")
    );
    let gate = level
        .nodes
        .iter()
        .find(|node| node.key.as_str() == "n_gate")
        .unwrap();
    assert_eq!(gate.hidden_prerequisites, [key("n_loose")]);
    let inside = |container: &str, top: &[NodeKind]| {
        let query = LevelQuery {
            container: Some(key(container)),
            top: top.iter().copied().collect(),
            ..every_kind()
        };
        level_of(&records, &query, &[]).nodes.len()
    };
    assert_eq!(
        inside("n_work", &[NodeKind::Deliverable]),
        0,
        "n_work's root is a group"
    );
    assert_eq!(inside("n_work", &[NodeKind::Group]), inside("n_work", &[]));
    assert_eq!(inside("n_work", &[]), 2);
}

/// C1: the level counts the nodes each class left out hides alone: not those under a collapse
/// or those the filter leaves out, and nothing when every class is shown.
#[test]
fn the_class_count_skips_collapsed_and_filtered_nodes() {
    let records = support::accepted(
        &journey(),
        "- op: answer\n  decision: n_ask\n  value: {boolean: false}\n",
    );
    let relevant = BTreeSet::from([LevelDisplay::Relevant, LevelDisplay::Conditional]);
    let count = |query: LevelQuery| {
        let level = level_of(
            &records,
            &LevelQuery {
                display: relevant.clone(),
                ..query
            },
            &[],
        );
        level
            .hidden_by_class
            .get(&LevelDisplay::NotRelevant)
            .copied()
            .unwrap_or(0)
    };
    assert_eq!(count(every_kind()), 2);
    assert_eq!(
        count(LevelQuery {
            collapsed: BTreeSet::from([key("n_work")]),
            ..every_kind()
        }),
        1,
        "the ruled-out action is under the collapse"
    );
    assert_eq!(
        count(searching("lighthouse")),
        1,
        "the filter keeps only the ruled-out spare"
    );
    assert!(
        level_of(&records, &every_kind(), &[])
            .hidden_by_class
            .is_empty()
    );
}

/// The level with `n_other` selected under `query`, checked to draw it with the containers above
/// it, none of them collapsed.
fn drawn_selected(records: &Records, query: LevelQuery) -> Level {
    let level = level_of(
        records,
        &LevelQuery {
            selected: Some(key("n_other")),
            ..query
        },
        &[],
    );
    let drawn = placement(&level);
    for node in ["n_other", "n_work", "n_group"] {
        assert!(drawn.contains_key(node), "{node} is drawn");
    }
    assert_eq!(drawn["n_other"], Some("n_work"));
    assert!(
        !level.collapsed.contains(&key("n_group")) && !level.collapsed.contains(&key("n_work")),
        "the containers above it are open"
    );
    level
}

/// C2, C7: the selected node, an action nothing else would draw, is drawn with the containers
/// above it, none of them collapsed, whatever hides it: its kind, its collapsed container, its
/// relevance class, a search it does not match, or the top-level rule. What else the rule
/// leaves out or hides stays so.
#[test]
fn the_selected_node_is_drawn_whatever_hides_it() {
    let records = journey();
    let kinds_without_actions: BTreeSet<NodeKind> = NodeKind::ALL
        .into_iter()
        .filter(|kind| *kind != NodeKind::Action)
        .collect();
    let by_kind = drawn_selected(&records, LevelQuery::of_kinds(kinds_without_actions, None));
    assert!(
        !placement(&by_kind).contains_key("n_step"),
        "the other action stays hidden"
    );
    assert_eq!(rolled_up(&by_kind, "n_work"), ["n_step"]);

    let by_collapse = drawn_selected(
        &records,
        LevelQuery {
            collapsed: BTreeSet::from([key("n_group")]),
            ..every_kind()
        },
    );
    assert!(
        placement(&by_collapse).contains_key("n_side"),
        "opening the containers above it draws what the rules draw beneath them"
    );

    let by_class = drawn_selected(
        &records,
        LevelQuery {
            display: BTreeSet::from([LevelDisplay::Relevant]),
            ..every_kind()
        },
    );
    assert!(
        !placement(&by_class).contains_key("n_spare"),
        "another conditional node stays hidden"
    );
    let by_text = drawn_selected(&records, searching("lighthouse"));
    assert!(
        !placement(&by_text).contains_key("n_side"),
        "no match, no draw"
    );

    let by_top = drawn_selected(
        &records,
        LevelQuery {
            top: BTreeSet::from([NodeKind::Milestone]),
            ..every_kind()
        },
    );
    let drawn = placement(&by_top);
    assert!(
        !drawn.contains_key("n_side") && !drawn.contains_key("n_spare"),
        "the rest of the left-out stage stays left out"
    );
    assert!(
        by_top
            .edges
            .iter()
            .all(|edge| edge.from.as_str() != "n_side" && edge.to.as_str() != "n_side"),
        "nothing re-targets onto the forced container"
    );
}

/// C2, C7: the hidden-by-class count leaves out the selected node alone.
#[test]
fn the_selected_node_is_left_out_of_the_hidden_count_alone() {
    let records = journey();
    let relevant_only = LevelQuery {
        display: BTreeSet::from([LevelDisplay::Relevant]),
        ..every_kind()
    };
    let hidden = drawn_selected(&records, relevant_only.clone())
        .hidden_by_class
        .get(&LevelDisplay::Conditional)
        .copied()
        .unwrap_or(0);
    let plain = level_of(&records, &relevant_only, &[]);
    assert_eq!(
        plain.hidden_by_class[&LevelDisplay::Conditional],
        hidden + 1,
        "the count leaves out the selected node alone"
    );
}

/// C2, C7: a selected container that is collapsed stays collapsed, and a selection outside the
/// drilled-in container, or an unknown one, changes nothing or fails like any unknown key.
#[test]
fn a_selected_container_stays_collapsed_and_a_selection_outside_changes_nothing() {
    let records = journey();
    let collapsed = LevelQuery {
        collapsed: BTreeSet::from([key("n_work")]),
        selected: Some(key("n_work")),
        ..every_kind()
    };
    let level = level_of(&records, &collapsed, &[]);
    assert!(level.collapsed.contains(&key("n_work")));
    assert!(!placement(&level).contains_key("n_step"));

    let drilled = LevelQuery {
        container: Some(key("n_work")),
        ..every_kind()
    };
    let outside = LevelQuery {
        selected: Some(key("n_loose")),
        ..drilled.clone()
    };
    assert_eq!(
        level_of(&records, &outside, &[]),
        level_of(&records, &drilled, &[])
    );

    let graph = support::journey_graph(&records, support::JOURNEY);
    let derived = support::derived(&records, support::JOURNEY);
    let unknown = LevelQuery {
        selected: Some(key("n_nothing")),
        ..every_kind()
    };
    assert!(
        DerivedJourney::new(&graph, &derived)
            .level(&unknown, &records.deployment, &BTreeSet::new())
            .is_err()
    );
}
