//! What inserting a route version asks of a person and does by default (B13, C19): where
//! each role and kind of the route lands, which of its nodes are left out, and which roots
//! are placed. The mutation's own mapping (`mutate::insertion`) applies the same defaults
//! to whatever it is not told, so this only reports them, and adds the one choice a default
//! cannot leave to chance: a route decision that fills a role the graph already fills is
//! left out, unless a condition of the route reads it.
//!
//! Cost: one pass over the route's roles, kinds and nodes per row, and one tree.

use std::collections::BTreeSet;

use cairn_schema::{
    Graph, InsertionAsk, InsertionFilled, InsertionPlan, InsertionRow, KindChoice, NodeKey,
    RoleChoice, RoleKey,
};

use crate::graph::Tree;
use crate::insertion::{default_kind, default_role};
use crate::mutate::insertion::excluded_nodes;
use crate::validate::state::role_filled_by;

/// B13, C19: the plan for inserting `route` into `host` given what the person has chosen.
#[must_use]
pub fn plan_insertion(host: &Graph, route: &Graph, ask: &InsertionAsk) -> InsertionPlan {
    let roles: Vec<InsertionRow> = route
        .roles
        .values()
        .map(|role| role_row(host, route, ask, role))
        .collect();
    let kinds = route
        .participation_kinds
        .values()
        .map(|kind| {
            let typical = default_kind(host, kind).map(|held| held.key.clone());
            let existing = match ask.kinds.get(&kind.key) {
                Some(KindChoice::Existing(key)) => Some(key.clone()),
                Some(KindChoice::Add) => None,
                None => typical,
            };
            InsertionRow {
                key: kind.key.to_string(),
                existing: existing.map(|key| key.to_string()),
                options: host
                    .participation_kinds
                    .values()
                    .filter(|held| held.multi == kind.multi)
                    .map(|held| held.key.to_string())
                    .collect(),
                filled: None,
            }
        })
        .collect();
    let omit: BTreeSet<NodeKey> = roles
        .iter()
        .filter_map(|row| row.filled.as_ref())
        .flat_map(|filled| filled.left_out.iter().cloned())
        .collect();
    let tree = Tree::build(route);
    let excluded = excluded_nodes(&tree, &omit);
    InsertionPlan {
        roles,
        kinds,
        omit: omit.into_iter().collect(),
        roots: tree
            .roots()
            .iter()
            .filter(|key| !excluded.contains(*key))
            .cloned()
            .collect(),
    }
}

fn role_row(
    host: &Graph,
    route: &Graph,
    ask: &InsertionAsk,
    role: &cairn_schema::Role<cairn_schema::KeyRefs>,
) -> InsertionRow {
    let deciders: Vec<&NodeKey> = route
        .nodes
        .values()
        .filter(|node| role_filled_by(node) == Some(&role.key))
        .map(|node| &node.key)
        .collect();
    let read = deciders
        .iter()
        .any(|decider| read_in_condition(route, decider));
    let typical = default_role(host, role).map(|held| held.key.clone());
    let existing = match ask.roles.get(&role.key) {
        Some(RoleChoice::Existing(key)) => Some(key.clone()),
        Some(RoleChoice::Add) => None,
        // A decision a condition reads stays, so an untouched row that would meet a filled
        // role becomes a new one instead.
        None => typical.filter(|key| !(read && filling(host, key).is_some())),
    };
    let filled = existing
        .as_ref()
        .and_then(|key| filling(host, key))
        .filter(|_| !read && !deciders.is_empty())
        .map(|by| InsertionFilled {
            by: match by {
                Filling::Decision(key) => Some(key),
                Filling::Direct => None,
            },
            left_out: deciders.iter().map(|key| (*key).clone()).collect(),
        });
    InsertionRow {
        key: role.key.to_string(),
        existing: existing.map(|key| key.to_string()),
        options: host
            .roles
            .values()
            .filter(|held| held.multi == role.multi)
            .map(|held| held.key.to_string())
            .collect(),
        filled,
    }
}

/// What fills a role in the graph.
enum Filling {
    /// A decision whose answer does.
    Decision(NodeKey),
    /// A direct fill kept in a journey's state.
    Direct,
}

fn filling(host: &Graph, role: &RoleKey) -> Option<Filling> {
    if let Some(node) = host
        .nodes
        .values()
        .find(|node| role_filled_by(node) == Some(role))
    {
        return Some(Filling::Decision(node.key.clone()));
    }
    host.state
        .role_fills
        .get(role)
        .is_some_and(|set| !set.is_empty())
        .then_some(Filling::Direct)
}

/// Whether a node of the route reads `decision` in a condition, so leaving it out would dangle.
fn read_in_condition(route: &Graph, decision: &NodeKey) -> bool {
    route.nodes.values().any(|node| {
        node.relevant_when
            .as_ref()
            .is_some_and(|condition| condition.decisions().contains(&decision))
    })
}

#[cfg(test)]
mod tests {
    use cairn_schema::{Graph, InsertionAsk, RoleChoice, RoleKey, SequentialKeys, from_yaml};

    use super::plan_insertion;

    fn graph(yaml: &str) -> Graph {
        let file = from_yaml(yaml).unwrap_or_else(|error| panic!("{error}\n{yaml}"));
        let graph = crate::from_file(&file, &mut SequentialKeys::default()).unwrap();
        graph.document().clone()
    }

    /// A route: a group holding the decision that fills `reviewer`, and a deliverable that
    /// reads that decision when `reads`; plus a second top-level action.
    fn route(reads: bool) -> Graph {
        let condition = if reads {
            "  relevant_when: {answered: who}\n"
        } else {
            ""
        };
        graph(&format!(
            "format: 1\nroute: r\nname: R\nroles:\n- id: reviewer\n  title: Reviewer\n\
             nodes:\n- id: who\n  kind: decision\n  title: Who\n  prompt: P\n  answer_type: identity\n  \
             fills_role: reviewer\n- id: model\n  kind: action\n  title: Model\n{condition}"
        ))
    }

    fn host(filled: bool) -> Graph {
        let fills = if filled {
            "  prompt: P\n  answer_type: identity\n  fills_role: reviewer\n"
        } else {
            "  prompt: P\n  answer_type: text\n"
        };
        graph(&format!(
            "format: 1\nroute: h\nname: H\nroles:\n- id: reviewer\n  title: Reviewer\n\
             nodes:\n- id: whois\n  kind: decision\n  title: Who is\n{fills}"
        ))
    }

    /// C19: a role the graph already fills leaves out the route's filling decision.
    #[test]
    fn a_filled_role_leaves_out_the_decision_that_fills_it() {
        let plan = plan_insertion(&host(true), &route(false), &InsertionAsk::default());
        let [row] = plan.roles.as_slice() else {
            panic!("{plan:?}")
        };
        assert!(row.existing.is_some());
        assert_eq!(row.filled.as_ref().unwrap().left_out.len(), 1);
        assert_eq!(plan.omit.len(), 1);
        // The decision is a root that is not placed; the action still is.
        assert_eq!(plan.roots.len(), 1);
    }

    /// C19: a role nothing fills keeps the route's decision.
    #[test]
    fn an_unfilled_role_keeps_the_decision() {
        let plan = plan_insertion(&host(false), &route(false), &InsertionAsk::default());
        assert_eq!(plan.omit, [] as [cairn_schema::NodeKey; 0]);
        assert_eq!(plan.roots.len(), 2);
        assert!(plan.roles[0].filled.is_none());
    }

    /// C19: a decision a condition reads stays, so an untouched row becomes a new role; one
    /// asked to be new does too.
    #[test]
    fn a_decision_a_condition_reads_stays() {
        let plan = plan_insertion(&host(true), &route(true), &InsertionAsk::default());
        assert_eq!(plan.omit, [] as [cairn_schema::NodeKey; 0]);
        assert_eq!(plan.roles[0].existing, None);

        let route = route(false);
        let key: RoleKey = route.roles.values().next().unwrap().key.clone();
        let ask = InsertionAsk {
            roles: [(key, RoleChoice::Add)].into(),
            ..InsertionAsk::default()
        };
        let plan = plan_insertion(&host(true), &route, &ask);
        assert_eq!(plan.omit, [] as [cairn_schema::NodeKey; 0]);
        assert_eq!(plan.roles[0].existing, None);
    }
}
