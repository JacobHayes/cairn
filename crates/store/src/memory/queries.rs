//! The memory backend's index and history queries: scans over the records, the reference
//! answers every backend's queries are held to.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    Domain, Entity, EntityKey, GraphId, JourneyId, ProposalStatus, RouteId, RouteKind,
    VersionNumber,
};

use super::State;
use crate::backend;
use crate::query::{
    DestinationKind, DestinationNow, ProposalCursor, ProposalPage, ProposalQuery, ProposalScope,
    ProposalSummary,
};
use crate::query::{
    EventQuery, JourneyMatches, JourneyQuery, JourneySummary, LoggedEvent, Page, RouteDetail,
    SearchHit, SearchQuery, VersionJourneys, text_matches,
};

impl State {
    fn latest_version(&self, route: &RouteId) -> Option<VersionNumber> {
        self.domains
            .versions
            .keys()
            .filter(|(held, _)| held == route)
            .map(|(_, version)| *version)
            .max()
    }

    pub(super) fn journeys(&self, query: &JourneyQuery) -> Page<JourneySummary, JourneyId> {
        let referencing = query
            .referencing
            .as_ref()
            .map(|entities| self.domains.journeys_referencing(entities));
        let mut items = Vec::new();
        for (id, (header, revision)) in &self.domains.journeys {
            let summary = JourneySummary {
                id: id.clone(),
                name: header.name.clone(),
                status: header.status,
                lineage: header.lineage.clone(),
                revision: *revision,
                created_at: header.created_at,
                latest_version: header
                    .lineage
                    .as_ref()
                    .and_then(|lineage| self.latest_version(&lineage.route)),
            };
            if backend::query::journey_listed(query, &summary, referencing.as_ref()) {
                items.push(summary);
                if items.len() > query.size.len() {
                    break;
                }
            }
        }
        Page::cut(items, query.size, |summary| summary.id.clone())
    }

    /// A destination as it is now.
    fn destination_now(&self, destination: &Domain) -> DestinationNow {
        match destination {
            Domain::Journey(id) => {
                let found = self.domains.journeys.get(id);
                DestinationNow {
                    exists: found.is_some(),
                    name: found.map(|(header, _)| header.name.clone()),
                    revision: found.map(|(_, revision)| *revision).unwrap_or_default(),
                    kind: DestinationKind::Journey,
                    status: found.map(|(header, _)| header.status),
                }
            }
            Domain::Route(id) => {
                let found = self.domains.routes.get(id);
                let kind = match found {
                    Some((header, _)) if header.kind == RouteKind::Segment => {
                        DestinationKind::Segment
                    }
                    _ => DestinationKind::Route,
                };
                DestinationNow {
                    exists: found.is_some(),
                    name: found.map(|(header, _)| header.name.clone()),
                    revision: found.map(|(_, revision)| *revision).unwrap_or_default(),
                    kind,
                    status: None,
                }
            }
            Domain::Deployment => DestinationNow {
                exists: true,
                name: None,
                revision: self.domains.deployment.revision,
                kind: DestinationKind::Deployment,
                status: None,
            },
        }
    }

    pub(super) fn proposals(&self, query: &ProposalQuery) -> ProposalPage {
        let waiting = match &query.scope {
            ProposalScope::Waiting { entities, .. } => Some(
                self.domains
                    .journeys_referencing(entities)
                    .into_keys()
                    .collect(),
            ),
            _ => None,
        };
        let candidates = self
            .domains
            .proposals
            .values()
            .filter(|proposal| proposal.status == ProposalStatus::Open)
            .map(|proposal| ProposalSummary {
                id: proposal.id.clone(),
                destination: proposal.destination.clone(),
                title: proposal.draft.title.clone(),
                revision: proposal.revision,
                drafted_against: proposal.draft.destination_revision,
                created_by: proposal.created_by.clone(),
                proposing_agent: proposal.proposing_agent.clone(),
                created_at: proposal.created_at,
                now: self.destination_now(&proposal.destination),
            })
            .collect();
        let destination = match &query.scope {
            ProposalScope::Destination(destination) => Some(self.destination_now(destination)),
            _ => None,
        };
        proposal_page(query, candidates, waiting.as_ref(), destination)
    }

    pub(super) fn route_detail(&self, route: &RouteId) -> Option<RouteDetail> {
        let (header, revision) = self.domains.routes.get(route)?;
        let mut uses = backend::query::insertion_uses(
            route,
            |held| self.latest_version(held),
            |version| {
                let id = GraphId::RouteVersion {
                    route: route.clone(),
                    version,
                };
                let root = self.domains.graphs.get(&id)?.nodes.values();
                root.into_iter()
                    .find(|node| node.parent.is_none())
                    .map(|node| node.key.clone())
            },
            self.domains.graphs.iter(),
        );
        let versions = self
            .domains
            .versions
            .iter()
            .filter(|((held, _), _)| held == route)
            .map(|((_, version), published_at)| VersionJourneys {
                insertions: uses.remove(version).unwrap_or_default(),
                version: *version,
                published_at: *published_at,
                journeys: self
                    .domains
                    .journeys
                    .iter()
                    .filter(|(_, (header, _))| {
                        header.lineage.as_ref().is_some_and(|lineage| {
                            lineage.route == *route && lineage.version == *version
                        })
                    })
                    .map(|(id, _)| id.clone())
                    .collect(),
            })
            .collect();
        Some(RouteDetail {
            header: header.clone(),
            revision: *revision,
            versions,
        })
    }

    pub(super) fn resolve_entity(&self, key: &EntityKey) -> Option<Entity> {
        let deployment = &self.domains.deployment;
        let key = deployment.aliases.get(key).unwrap_or(key);
        deployment.entities.get(key).cloned()
    }

    pub(super) fn events(&self, query: &EventQuery) -> Page<LoggedEvent, u64> {
        let mut items = Vec::new();
        for logged in &self.events {
            let event = &logged.event;
            let keep = query.after.is_none_or(|after| logged.seq > after)
                && query.log.as_ref().is_none_or(|log| event.log == *log)
                && query
                    .user
                    .as_ref()
                    .is_none_or(|user| event.actor.user == *user)
                && (query.types.is_empty() || query.types.contains(&event.event_type))
                && query
                    .patch
                    .as_ref()
                    .is_none_or(|patch| event.patch_id == *patch)
                && query.from.is_none_or(|from| event.at >= from)
                && query.until.is_none_or(|until| event.at < until)
                && query.node.as_ref().is_none_or(|node| {
                    self.event_nodes
                        .get(&logged.seq)
                        .is_some_and(|nodes| nodes.contains(node))
                });
            if keep {
                items.push(logged.clone());
                if items.len() > query.size.len() {
                    break;
                }
            }
        }
        Page::cut(items, query.size, |logged| logged.seq)
    }

    pub(super) fn search(&self, query: &SearchQuery) -> Page<JourneyMatches, JourneyId> {
        let needle = query.text.as_str();
        let mut items = Vec::new();
        for (id, (header, _)) in &self.domains.journeys {
            if query.after.as_ref().is_some_and(|after| id <= after) {
                continue;
            }
            let mut hits = BTreeSet::new();
            if text_matches(header.name.as_str(), needle) {
                hits.insert(SearchHit::JourneyName);
            }
            if let Some(description) = &header.description
                && text_matches(description.as_str(), needle)
            {
                hits.insert(SearchHit::JourneyDescription);
            }
            hits.extend(graph_hits(
                &self.domains.graph(&GraphId::Journey(id.clone())),
                needle,
            ));
            if !hits.is_empty() {
                items.push(JourneyMatches {
                    journey: id.clone(),
                    name: header.name.clone(),
                    hits,
                });
                if items.len() > query.size.len() {
                    break;
                }
            }
        }
        Page::cut(items, query.size, |matches| matches.journey.clone())
    }
}

/// Where `needle` appears in a journey's graph.
fn graph_hits(graph: &cairn_schema::Graph, needle: &str) -> BTreeSet<SearchHit> {
    let mut hits = BTreeSet::new();
    for node in graph.nodes.values() {
        if text_matches(node.title.as_str(), needle) {
            hits.insert(SearchHit::NodeTitle(node.key.clone()));
        }
        if let Some(description) = &node.description
            && text_matches(description.as_str(), needle)
        {
            hits.insert(SearchHit::NodeDescription(node.key.clone()));
        }
        for resource in &node.resources {
            if text_matches(&backend::query::resource_text(resource), needle) {
                hits.insert(SearchHit::Resource {
                    node: node.key.clone(),
                    resource: resource.key.clone(),
                });
            }
        }
    }
    for annotation in graph.state.annotations.values() {
        if text_matches(&backend::query::annotation_text(annotation), needle) {
            hits.insert(SearchHit::Annotation(annotation.body.key.clone()));
        }
    }
    hits
}

/// The journeys referencing entities, for the trait's query.
pub(super) fn referencing(
    state: &State,
    entities: &BTreeSet<EntityKey>,
) -> BTreeMap<JourneyId, cairn_schema::Revision> {
    state.domains.journeys_referencing(entities)
}

/// C14, C16: whether an open proposal belongs in a listing's scope. `waiting` is the journeys
/// referring to the scope's entities, when the scope is the viewer's waiting list.
fn proposal_listed(
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

/// C14: the page of a listing, from every open proposal `candidates`: the scope's members,
/// newest first by creation time then id, after the cursor, with the scope's whole count. The
/// reference the Turso backend's SQL answers alike (the conformance suite holds them to it).
fn proposal_page(
    query: &ProposalQuery,
    candidates: Vec<ProposalSummary>,
    waiting: Option<&BTreeSet<JourneyId>>,
    destination: Option<DestinationNow>,
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
        listed.last().map(|last| ProposalCursor {
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
