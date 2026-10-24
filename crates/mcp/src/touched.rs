//! What a stale refusal or an applied-over answer says was touched, grouped so an agent reads
//! it once: by the graph or domain the records are in, then by the kind of record, then the
//! subjects. The same key in two graphs stays distinct, because the graph is the outer key.

use std::collections::BTreeMap;

use cairn_schema::{RecordKey, TouchedSet};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Record addresses by scope (`journey j_a`, `route_version r/1`), then record kind
/// (`node_state`, `answer`), each with the subjects it names (node keys, or a node and what
/// is on it). A kind with no subject, such as a whole graph, lists none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub(crate) struct Touched(BTreeMap<String, BTreeMap<String, Vec<String>>>);

impl Touched {
    /// The addresses in `set`, grouped.
    pub(crate) fn of(set: &TouchedSet) -> Self {
        let mut grouped: BTreeMap<String, BTreeMap<String, Vec<String>>> = BTreeMap::new();
        for key in set.as_set() {
            let (scope, kind, subject) = describe(key);
            let subjects = grouped.entry(scope).or_default().entry(kind).or_default();
            if !subject.is_empty() {
                subjects.push(subject);
            }
        }
        Self(grouped)
    }
}

/// The scope, kind and subject of one address, read from its wire form so a record kind added
/// later is grouped without a change here.
fn describe(key: &RecordKey) -> (String, String, String) {
    let wire = serde_json::to_value(key).unwrap_or(Value::Null);
    if let RecordKey::InGraph { graph, .. } = key {
        let inner = wire
            .get("in_graph")
            .and_then(|inner| inner.get("key"))
            .cloned()
            .unwrap_or(Value::Null);
        let graph = serde_json::to_value(graph).unwrap_or(Value::Null);
        let (kind, subject) = tagged(inner);
        return (label(graph), kind, subject);
    }
    let (kind, subject) = tagged(wire);
    (key.domain().to_string(), kind, subject)
}

/// `kind id` for an externally tagged value.
fn label(value: Value) -> String {
    let (kind, subject) = tagged(value);
    if subject.is_empty() {
        kind
    } else {
        format!("{kind} {subject}")
    }
}

/// The variant name and the text of its payload.
fn tagged(value: Value) -> (String, String) {
    match value {
        Value::String(kind) => (kind, String::new()),
        Value::Object(map) => match map.into_iter().next() {
            Some((kind, payload)) => (kind, leaf(payload)),
            None => (String::new(), String::new()),
        },
        other => (String::new(), leaf(other)),
    }
}

/// A payload as text: a string as it is, an object's values joined by `/`, with `node` first.
fn leaf(value: Value) -> String {
    match value {
        Value::String(text) => text,
        Value::Object(mut map) => {
            let node = map.remove("node").map(leaf);
            let rest = map.into_iter().map(|(_, value)| leaf(value));
            node.into_iter().chain(rest).collect::<Vec<_>>().join("/")
        }
        Value::Array(items) => items.into_iter().map(leaf).collect::<Vec<_>>().join(","),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use cairn_schema::{GraphId, GraphKey};

    use super::*;

    fn in_graph(graph: GraphId, key: GraphKey) -> RecordKey {
        RecordKey::InGraph { graph, key }
    }

    #[test]
    fn the_same_key_in_two_graphs_stays_distinct_and_keys_group_by_kind() {
        let journey: GraphId = GraphId::Journey("j_a".parse().unwrap());
        let draft = GraphId::RouteDraft("r-a".parse().unwrap());
        let node = || -> cairn_schema::NodeKey { "n_x".parse().unwrap() };
        let other = || -> cairn_schema::NodeKey { "n_y".parse().unwrap() };
        let set: TouchedSet = [
            in_graph(journey.clone(), GraphKey::NodeState(node())),
            in_graph(journey.clone(), GraphKey::NodeState(other())),
            in_graph(journey, GraphKey::Answer(node())),
            in_graph(draft, GraphKey::NodeState(node())),
        ]
        .into_iter()
        .collect();
        let grouped = serde_json::to_value(Touched::of(&set)).unwrap();
        assert_eq!(
            grouped,
            serde_json::json!({
                "journey j_a": { "node_state": ["n_x", "n_y"], "answer": ["n_x"] },
                "route_draft r-a": { "node_state": ["n_x"] },
            })
        );
    }
}
