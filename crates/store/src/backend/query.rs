//! The read side of the commit rules, shared by the memory and Turso backends: which entities
//! a graph refers to, which nodes an event is about, the text a search matches, which
//! journeys an index lists, and where a segment is inserted.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    Domain, EntityKey, Graph, GraphId, InsertionKey, JourneyId, NodeKey, RecordKey, Revision,
    RouteId, Subject, VersionNumber, Write,
};

use crate::query::{InsertionUse, ProposalPage, ProposalQuery, ProposalScope, ProposalSummary};

/// E6: the entities a journey's graph refers to, by the key it holds (an entity or an
/// alias): answers, direct role fills, explicit participations, and condition values that
/// name an entity key. A condition value is text the decision's answer type gives meaning,
/// so any value shaped like an entity key counts: a superset only costs a merge an extra
/// journey to check.
#[must_use]
pub fn entity_references(graph: &Graph) -> BTreeSet<EntityKey> {
    use cairn_schema::{AnswerValue, ParticipationSource};
    let mut keys = BTreeSet::new();
    for answer in graph.state.answers.values() {
        match answer {
            AnswerValue::Entity(key) => {
                keys.insert(key.clone());
            }
            AnswerValue::EntityList(set) => keys.extend(set.iter().cloned()),
            _ => {}
        }
    }
    for fill in graph.state.role_fills.values() {
        keys.extend(fill.iter().cloned());
    }
    for node in graph.nodes.values() {
        for source in node.participations.as_map().values() {
            if let ParticipationSource::Entities(set) = source {
                keys.extend(set.iter().cloned());
            }
        }
        if let Some(condition) = &node.relevant_when
            && let Ok(value) = serde_json::to_value(condition)
        {
            keys.extend(entity_keys_in(&value));
        }
    }
    keys
}

/// Every string in a JSON value that parses as an entity key, walked with an explicit stack
/// (PRACTICES, No recursion).
#[must_use]
pub(super) fn entity_keys_in(value: &serde_json::Value) -> BTreeSet<EntityKey> {
    let mut keys = BTreeSet::new();
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::String(text) => keys.extend(text.parse::<EntityKey>().ok()),
            serde_json::Value::Array(items) => stack.extend(items),
            serde_json::Value::Object(fields) => stack.extend(fields.values()),
            _ => {}
        }
    }
    keys
}

/// E6: the keys that name the same entities as `entities`: each one's entity (an alias
/// resolves to the entity it was merged into), and every key naming one of those entities,
/// itself or an alias of it.
#[must_use]
pub fn with_aliases(
    entities: &BTreeSet<EntityKey>,
    aliases: &BTreeMap<EntityKey, EntityKey>,
) -> BTreeSet<EntityKey> {
    let resolved: BTreeSet<EntityKey> = entities
        .iter()
        .map(|key| aliases.get(key).unwrap_or(key).clone())
        .collect();
    let mut keys = entities.clone();
    keys.extend(resolved.iter().cloned());
    for (alias, entity) in aliases {
        if resolved.contains(entity) {
            keys.insert(alias.clone());
        }
    }
    keys
}

/// J5: the nodes an event is about or wrote on: its subject, the node every record it wrote
/// hangs off, and both ends of an edge. The event filter by node reads it.
#[must_use]
pub fn event_nodes(event: &cairn_schema::Event) -> BTreeSet<NodeKey> {
    let mut nodes = BTreeSet::new();
    if let Subject::Node(node) = &event.subject {
        nodes.insert(node.clone());
    }
    if let Subject::Edge(edge) = &event.subject {
        nodes.insert(edge.node.clone());
        nodes.insert(edge.requires.clone());
    }
    for key in event.touched().as_set() {
        if let RecordKey::InGraph { key, .. } = key {
            nodes.extend(key.node_scope().cloned());
            if let cairn_schema::GraphKey::Edge(edge) = key {
                nodes.insert(edge.requires.clone());
            }
        }
    }
    nodes
}

/// The whole nodes an event removes, by graph: a removed node takes the edges into it, so
/// the nodes that required it are on the event too (J5, a node's history). The backend looks
/// them up in the graph as it was before the commit.
#[must_use]
pub fn removed_nodes(event: &cairn_schema::Event) -> Vec<(GraphId, NodeKey)> {
    event
        .delta
        .iter()
        .filter_map(|write| match write {
            Write::Remove(RecordKey::InGraph {
                graph,
                key: cairn_schema::GraphKey::Node(node),
            }) => Some((graph.clone(), node.clone())),
            _ => None,
        })
        .collect()
}

/// The text a resource is searched by: its title and its content as written.
#[must_use]
pub fn resource_text(resource: &cairn_schema::Resource<cairn_schema::refs::KeyRefs>) -> String {
    written_text(resource)
}

/// The text a note or link is searched by: its title and its content as written.
#[must_use]
pub fn annotation_text(annotation: &cairn_schema::Annotation) -> String {
    written_text(&annotation.body)
}

/// Every string field of a value's written form but its key and node, joined by line
/// breaks so a match never spans two fields.
fn written_text<T: serde::Serialize>(value: &T) -> String {
    let Ok(serde_json::Value::Object(fields)) = serde_json::to_value(value) else {
        return String::new();
    };
    fields
        .iter()
        .filter(|(name, _)| !matches!(name.as_str(), "key" | "node"))
        .filter_map(|(_, value)| value.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// C16: whether a journey belongs in the index `query` asks for. `referencing` is the
/// journeys referring to the query's entities, when it names any.
#[must_use]
pub fn journey_listed(
    query: &crate::query::JourneyQuery,
    summary: &crate::query::JourneySummary,
    referencing: Option<&BTreeMap<JourneyId, Revision>>,
) -> bool {
    let route = summary.lineage.as_ref().map(|lineage| &lineage.route);
    let version = summary.lineage.as_ref().map(|lineage| lineage.version);
    query.after.as_ref().is_none_or(|after| summary.id > *after)
        && (query.statuses.is_empty() || query.statuses.contains(&summary.status))
        && query
            .route
            .as_ref()
            .is_none_or(|wanted| route == Some(wanted))
        && query.version.is_none_or(|wanted| version == Some(wanted))
        && referencing.is_none_or(|found| found.contains_key(&summary.id))
        && query
            .upgrade_available
            .is_none_or(|wanted| summary.upgrade_available() == wanted)
}

/// C19: the insertions of `segment`'s versions across `graphs`, each under the version it is on:
/// journeys, route drafts, and each route's latest version, where an insertion a draft also
/// holds is listed once, as the draft's. `latest` is a route's latest published version, and
/// `root` the key of a version of `segment`'s one root. Cost: one pass over the insertions of
/// the graphs given.
#[must_use]
pub fn insertion_uses<'g>(
    segment: &RouteId,
    latest: impl Fn(&RouteId) -> Option<VersionNumber>,
    root: impl Fn(VersionNumber) -> Option<NodeKey>,
    graphs: impl IntoIterator<Item = (&'g GraphId, &'g Graph)>,
) -> BTreeMap<VersionNumber, Vec<InsertionUse>> {
    let graphs: Vec<_> = graphs.into_iter().collect();
    let drafted: BTreeSet<(&RouteId, &InsertionKey)> = graphs
        .iter()
        .filter_map(|(id, graph)| match id {
            GraphId::RouteDraft(route) => Some((route, graph)),
            _ => None,
        })
        .flat_map(|(route, graph)| graph.insertions.values().map(move |i| (route, &i.key)))
        .collect();
    let newest = latest(segment);
    let mut found: BTreeMap<VersionNumber, Vec<InsertionUse>> = BTreeMap::new();
    for (id, graph) in graphs {
        let (host, current) = match id {
            GraphId::Journey(journey) => (Domain::Journey(journey.clone()), true),
            GraphId::RouteDraft(route) => (Domain::Route(route.clone()), true),
            GraphId::RouteVersion { route, version } => (
                Domain::Route(route.clone()),
                latest(route) == Some(*version),
            ),
        };
        for insertion in graph.insertions.values() {
            let held_by_draft = matches!(id, GraphId::RouteVersion { route, .. }
                if drafted.contains(&(route, &insertion.key)));
            if insertion.segment.route != *segment || !current || held_by_draft {
                continue;
            }
            // Members may move, so the root is the member copied from the version's root; once
            // it is removed, the insertion is listed under what is left of it.
            let version_root = root(insertion.segment.version);
            let members = insertion.nodes.iter();
            let copied = members
                .clone()
                .find(|(_, from)| Some(*from) == version_root.as_ref());
            let root = copied
                .or_else(|| members.clone().next())
                .and_then(|(member, _)| graph.nodes.get(member));
            let Some(root) = root else { continue };
            found
                .entry(insertion.segment.version)
                .or_default()
                .push(InsertionUse {
                    host: host.clone(),
                    graph: id.clone(),
                    insertion: insertion.key.clone(),
                    title: root.title.clone(),
                    upgrade_available: newest
                        .is_some_and(|newest| newest > insertion.segment.version),
                });
        }
    }
    for uses in found.values_mut() {
        uses.sort_by(|a, b| (&a.graph, &a.insertion).cmp(&(&b.graph, &b.insertion)));
    }
    found
}

/// C14, C16: whether an open proposal belongs in a listing's scope. `waiting` is the journeys
/// referring to the scope's entities, when the scope is the viewer's waiting list.
#[must_use]
pub fn proposal_listed(
    scope: &ProposalScope,
    summary: &ProposalSummary,
    waiting: Option<&BTreeSet<JourneyId>>,
) -> bool {
    match scope {
        ProposalScope::Destination(destination) => summary.destination == *destination,
        ProposalScope::CreatingJourneys => {
            matches!(summary.destination, Domain::Journey(_)) && !summary.now.exists
        }
        ProposalScope::CreatingRoutes => {
            matches!(summary.destination, Domain::Route(_)) && !summary.now.exists
        }
        ProposalScope::Waiting { user, .. } => {
            summary.created_by == *user
                || match (&summary.destination, waiting) {
                    (Domain::Journey(journey), Some(found)) => {
                        summary.proposing_agent.is_some()
                            && summary.now.exists
                            && summary.now.status == Some(cairn_schema::JourneyStatus::Active)
                            && found.contains(journey)
                    }
                    _ => false,
                }
        }
    }
}

/// C14: the page of a listing, from every open proposal `candidates` the backend read: the
/// scope's members, newest first by creation time then id, after the cursor, with the
/// scope's whole count. Every backend builds its page here, so they order and cut alike.
#[must_use]
pub fn proposal_page(
    query: &ProposalQuery,
    candidates: Vec<ProposalSummary>,
    waiting: Option<&BTreeSet<JourneyId>>,
    destination: Option<crate::query::DestinationNow>,
) -> ProposalPage {
    let mut listed: Vec<_> = candidates
        .into_iter()
        .filter(|summary| proposal_listed(&query.scope, summary, waiting))
        .collect();
    listed.sort_by(|left, right| (right.created_at, &right.id).cmp(&(left.created_at, &left.id)));
    let count = u32::try_from(listed.len()).unwrap_or(u32::MAX);
    if let Some(after) = &query.after {
        listed.retain(|summary| (summary.created_at, &summary.id) < (after.created_at, &after.id));
    }
    let more = listed.len() > query.size.len();
    listed.truncate(query.size.len());
    let next = if more {
        listed.last().map(|last| crate::query::ProposalCursor {
            created_at: last.created_at,
            id: last.id.clone(),
        })
    } else {
        None
    };
    ProposalPage {
        count,
        destination,
        items: listed,
        next,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::{self, id};
    use cairn_schema::EventType;

    #[test]
    fn entity_references_reach_conditions() {
        let mut graph = Graph::default();
        graph
            .nodes
            .put(build::node(serde_json::json!({
                "key": "n_a", "id": "a", "kind": "action", "title": "A",
                "relevant_when": {"equals": {"decision": "n_d", "value": "e_alice"}},
                "participations": {"k_owner": ["e_bob"]},
            })))
            .unwrap();
        assert_eq!(
            entity_references(&graph),
            BTreeSet::from([id("e_alice"), id("e_bob")])
        );
    }

    #[test]
    fn an_event_is_about_its_subject_and_the_nodes_it_wrote_on() {
        let graph = build::journey_graph("j_one");
        let commit = build::journey_patch("p_one", "j_one", 1)
            .event(
                EventType::EdgeChanged,
                Subject::Edge(cairn_schema::Edge {
                    node: id("n_b"),
                    requires: id("n_a"),
                }),
                vec![build::put_in(
                    &graph,
                    cairn_schema::GraphRecord::Edge(cairn_schema::Edge {
                        node: id("n_b"),
                        requires: id("n_a"),
                    }),
                )],
            )
            .event(
                EventType::AnnotationAdded,
                Subject::Attachment(id("a_note")),
                vec![build::put_in(
                    &graph,
                    cairn_schema::GraphRecord::Annotation(build::note("a_note", Some("n_c"), "x")),
                )],
            )
            .commit();
        let events = &commit.change_set.events;
        assert_eq!(
            event_nodes(&events[0]),
            BTreeSet::from([id("n_a"), id("n_b")])
        );
        assert_eq!(event_nodes(&events[1]), BTreeSet::from([id("n_c")]));
    }
}
