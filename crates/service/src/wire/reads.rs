//! The store-backed reads' wire shapes: the journey index (C16), route detail (C17), events
//! (J5), and search, each page carrying where the next one starts.

use std::collections::BTreeSet;

use cairn_schema::{
    AttachmentKey, Domain, Event, GraphId, InsertionKey, JourneyId, JourneyStatus, Lineage,
    NodeKey, Revision, RouteHeader, RouteId, Timestamp, Title, VersionNumber,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// PRD: C16.
/// A journey in the index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JourneySummary {
    /// The journey.
    pub id: JourneyId,
    /// Its name.
    pub name: Title,
    /// Its status.
    pub status: JourneyStatus,
    /// The route version it follows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lineage: Option<Lineage>,
    /// Its revision.
    pub revision: Revision,
    /// When it was created.
    pub created_at: Timestamp,
    /// The latest version its route has published.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<VersionNumber>,
    // PRD: C16.
    /// Whether its route has published a version newer than its own.
    pub upgrade_available: bool,
}

impl From<cairn_store::Page<cairn_store::JourneySummary, JourneyId>> for JourneyPage {
    fn from(page: cairn_store::Page<cairn_store::JourneySummary, JourneyId>) -> Self {
        Self {
            items: page.items.into_iter().map(Into::into).collect(),
            next: page.next,
        }
    }
}

impl From<cairn_store::JourneySummary> for JourneySummary {
    fn from(summary: cairn_store::JourneySummary) -> Self {
        let upgrade_available = summary.upgrade_available();
        let cairn_store::JourneySummary {
            id,
            name,
            status,
            lineage,
            revision,
            created_at,
            latest_version,
        } = summary;
        Self {
            id,
            name,
            status,
            lineage,
            revision,
            created_at,
            latest_version,
            upgrade_available,
        }
    }
}

/// A page of the journey index; `next` is where the next page starts, absent on the last.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JourneyPage {
    /// The journeys, in id order.
    pub items: Vec<JourneySummary>,
    /// Pass as `after` for the next page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<JourneyId>,
}

// PRD: I2.
/// A route in the route index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteSummary {
    /// The route's fields.
    pub header: RouteHeader,
    /// Its revision.
    pub revision: Revision,
    /// Its latest published version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<VersionNumber>,
    /// Whether it has an open draft.
    pub draft_open: bool,
}

impl From<cairn_store::Page<crate::RouteSummary, RouteId>> for RoutePage {
    fn from(page: cairn_store::Page<crate::RouteSummary, RouteId>) -> Self {
        Self {
            items: page.items.into_iter().map(Into::into).collect(),
            next: page.next,
        }
    }
}

impl From<crate::RouteSummary> for RouteSummary {
    fn from(summary: crate::RouteSummary) -> Self {
        let crate::RouteSummary {
            header,
            revision,
            latest_version,
            draft_open,
        } = summary;
        Self {
            header,
            revision,
            latest_version,
            draft_open,
        }
    }
}

/// A page of the route index; `next` is where the next page starts, absent on the last.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RoutePage {
    /// The routes, in id order.
    pub items: Vec<RouteSummary>,
    /// Pass as `after` for the next page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<RouteId>,
}

// PRD: C17.
/// A route's published versions with the journeys on each.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteDetail {
    /// The route's fields.
    pub header: RouteHeader,
    /// Its revision.
    pub revision: Revision,
    /// Its versions, oldest first.
    pub versions: Vec<VersionJourneys>,
    // PRD: A19.
    /// Whether anything uses it: a journey following one of its versions, or an insertion of
    /// one anywhere. A route in use cannot be deleted.
    pub in_use: bool,
}

/// One published version and the journeys on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VersionJourneys {
    /// The version.
    pub version: VersionNumber,
    /// When it was published.
    pub published_at: Timestamp,
    /// The journeys whose lineage is this version.
    pub journeys: BTreeSet<JourneyId>,
    // PRD: C19.
    /// Where this version of a segment is inserted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub insertions: Vec<InsertionUse>,
}

// PRD: C19.
/// One insertion of a segment version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InsertionUse {
    /// The domain holding it.
    pub host: Domain,
    /// The graph holding it.
    pub graph: GraphId,
    /// The insertion.
    pub insertion: InsertionKey,
    /// The root member's current title.
    pub title: Title,
    /// Whether the segment has a later published version.
    pub upgrade_available: bool,
}

impl From<cairn_store::RouteDetail> for RouteDetail {
    fn from(detail: cairn_store::RouteDetail) -> Self {
        let cairn_store::RouteDetail {
            header,
            revision,
            versions,
            in_use,
        } = detail;
        let versions = versions.into_iter().map(|version| {
            let cairn_store::VersionJourneys {
                version,
                published_at,
                journeys,
                insertions,
            } = version;
            VersionJourneys {
                version,
                published_at,
                journeys,
                insertions: insertions
                    .into_iter()
                    .map(|held| {
                        let cairn_store::InsertionUse {
                            host,
                            graph,
                            insertion,
                            title,
                            upgrade_available,
                        } = held;
                        InsertionUse {
                            host,
                            graph,
                            insertion,
                            title,
                            upgrade_available,
                        }
                    })
                    .collect(),
            }
        });
        Self {
            header,
            revision,
            versions: versions.collect(),
            in_use,
        }
    }
}

// PRD: J5.
/// An event and its position in the log, which orders history and pages it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LoggedEvent {
    /// The position: increasing in commit order.
    pub seq: u64,
    /// The event.
    pub event: Event,
}

/// A page of events; `next` is where the next page starts, absent on the last.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EventPage {
    /// The events, in commit order.
    pub items: Vec<LoggedEvent>,
    /// Pass as `after` for the next page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<u64>,
}

impl From<cairn_store::Page<cairn_store::LoggedEvent, u64>> for EventPage {
    fn from(page: cairn_store::Page<cairn_store::LoggedEvent, u64>) -> Self {
        let items = page.items.into_iter().map(|logged| LoggedEvent {
            seq: logged.seq,
            event: logged.event,
        });
        Self {
            items: items.collect(),
            next: page.next,
        }
    }
}

/// Where search text was found in a journey.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(tag = "in", rename_all = "snake_case", deny_unknown_fields)]
pub enum SearchHit {
    /// The journey's name.
    JourneyName,
    /// The journey's description.
    JourneyDescription,
    /// A node's title.
    NodeTitle {
        /// The node.
        node: NodeKey,
    },
    /// A node's description.
    NodeDescription {
        /// The node.
        node: NodeKey,
    },
    /// A note or link: its title or content.
    Annotation {
        /// The note or link.
        annotation: AttachmentKey,
    },
    /// A resource on a node: its title or content.
    Resource {
        /// The node.
        node: NodeKey,
        /// The resource.
        resource: AttachmentKey,
    },
}

impl From<cairn_store::SearchHit> for SearchHit {
    fn from(hit: cairn_store::SearchHit) -> Self {
        match hit {
            cairn_store::SearchHit::JourneyName => SearchHit::JourneyName,
            cairn_store::SearchHit::JourneyDescription => SearchHit::JourneyDescription,
            cairn_store::SearchHit::NodeTitle(node) => SearchHit::NodeTitle { node },
            cairn_store::SearchHit::NodeDescription(node) => SearchHit::NodeDescription { node },
            cairn_store::SearchHit::Annotation(annotation) => SearchHit::Annotation { annotation },
            cairn_store::SearchHit::Resource { node, resource } => {
                SearchHit::Resource { node, resource }
            }
        }
    }
}

/// One journey's search matches.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JourneyMatches {
    /// The journey.
    pub journey: JourneyId,
    /// Its name.
    pub name: Title,
    /// Where the text was found, in order.
    pub hits: Vec<SearchHit>,
}

/// A page of search results; `next` is where the next page starts, absent on the last.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchPage {
    /// The journeys with matches, in id order.
    pub items: Vec<JourneyMatches>,
    /// Pass as `after` for the next page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<JourneyId>,
}

impl From<cairn_store::Page<cairn_store::JourneyMatches, JourneyId>> for SearchPage {
    fn from(page: cairn_store::Page<cairn_store::JourneyMatches, JourneyId>) -> Self {
        let items = page.items.into_iter().map(|matches| JourneyMatches {
            journey: matches.journey,
            name: matches.name,
            hits: matches.hits.into_iter().map(SearchHit::from).collect(),
        });
        Self {
            items: items.collect(),
            next: page.next,
        }
    }
}

// PRD: E6.
/// Where a route refers to an identity: its draft, and the published versions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteReference {
    /// The route's fields.
    pub header: RouteHeader,
    /// Whether its draft refers to the identity.
    pub draft: bool,
    /// The published versions that refer to it, ascending.
    pub versions: Vec<VersionNumber>,
}

// PRD: E6.
/// Everything that refers to an identity, directly or through an alias: every journey,
/// whatever its status, and every route whose draft or published versions do. Authoritative:
/// an identity with neither can be deleted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IdentityReferences {
    /// The journeys, in id order.
    pub journeys: Vec<JourneySummary>,
    /// The routes, in id order.
    pub routes: Vec<RouteReference>,
}

impl From<crate::IdentityReferences> for IdentityReferences {
    fn from(references: crate::IdentityReferences) -> Self {
        Self {
            journeys: references.journeys.into_iter().map(Into::into).collect(),
            routes: references
                .routes
                .into_iter()
                .map(|route| RouteReference {
                    header: route.header,
                    draft: route.draft,
                    versions: route.versions,
                })
                .collect(),
        }
    }
}
