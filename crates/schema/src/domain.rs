//! The patch domains (A17; ARCHITECTURE, Terms: Domain): a journey, a route with its draft,
//! and the deployment, each loaded and revision-checked as one unit, plus the route
//! versions a route publishes, which are immutable graphs loaded one at a time.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::collections::{ByDocumentSize, HasKey, Keyed};
use crate::graph::Graph;
use crate::id::{EntityKey, JourneyId, RouteId};
use crate::number::{Revision, VersionNumber};
use crate::text::{Email, Markdown, Title};
use jiff::Timestamp;
use jiff::civil::Date;

/// A patch domain, by identity (A17).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Domain {
    /// A journey.
    Journey(JourneyId),
    /// A route, including its draft.
    Route(RouteId),
    /// The deployment: its entities and aliases.
    Deployment,
}

impl fmt::Display for Domain {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Domain::Journey(id) => write!(formatter, "journey {id}"),
            Domain::Route(id) => write!(formatter, "route {id}"),
            Domain::Deployment => formatter.write_str("deployment"),
        }
    }
}

/// Which graph a stored record belongs to (ARCHITECTURE, Schema outline: `graphs`).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum GraphId {
    /// A journey's graph.
    Journey(JourneyId),
    /// A route's draft.
    RouteDraft(RouteId),
    /// A published route version.
    RouteVersion {
        /// The route.
        route: RouteId,
        /// The version.
        version: VersionNumber,
    },
}

/// A journey's link to the route version it was created from or last upgraded to
/// (PRD glossary, Lineage).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Lineage {
    /// The route.
    pub route: RouteId,
    /// The version.
    pub version: VersionNumber,
}

/// A journey's status (B11).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum JourneyStatus {
    /// In progress.
    Active,
    /// Marked complete; still patchable, out of "mine" and cross-journey lists.
    Completed,
    /// Archived: accepts only un-archiving or hard deletion.
    Archived,
}

/// A journey's own fields, apart from its graph.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JourneyHeader {
    /// The journey's id.
    pub id: JourneyId,
    /// Its name.
    pub name: Title,
    /// What it is for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Markdown>,
    /// Its status.
    pub status: JourneyStatus,
    /// The route version it follows, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lineage: Option<Lineage>,
    /// When it was created.
    pub created_at: Timestamp,
    /// The deployment-local calendar day it was created, the `created_at` date rules
    /// measure from (A8, A9). Stored beside the timestamp so the engine never converts time
    /// zones.
    pub created_on: Date,
}

/// A journey domain: its fields, revision, and graph with state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Journey {
    /// The journey's own fields.
    pub header: JourneyHeader,
    /// The journey revision (H5).
    pub revision: Revision,
    /// The graph and its state.
    pub graph: Graph,
}

/// A route's own fields, apart from its graphs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteHeader {
    /// The route's id.
    pub id: RouteId,
    /// Its name.
    pub name: Title,
    /// What it is for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Markdown>,
    /// Hidden from new-journey creation (A19); its journeys still see upgrades.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub retired: bool,
}

/// A route's single mutable draft (A11).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteDraft {
    /// The latest published version when the draft was opened, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<VersionNumber>,
    /// The draft graph (no state).
    pub graph: Graph,
}

/// A route domain: its fields, revision, published version numbers, and draft. Versions
/// themselves are separate graphs, never part of the domain load.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Route {
    /// The route's own fields.
    pub header: RouteHeader,
    /// The route revision (H5), covering the draft.
    pub revision: Revision,
    /// The published versions.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub versions: BTreeSet<VersionNumber>,
    /// The draft, if one is open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<RouteDraft>,
}

/// An immutable published route version (A11).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteVersion {
    /// The route.
    pub route: RouteId,
    /// The version number.
    pub version: VersionNumber,
    /// When it was published.
    pub published_at: Timestamp,
    /// The graph (no state).
    pub graph: Graph,
}

/// A person or team journeys refer to (PRD glossary, Entity), deployment-scoped.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Entity {
    /// The entity's key.
    pub key: EntityKey,
    /// Its name.
    pub name: Title,
    /// Its emails, each held by at most one entity (H3).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub emails: BTreeSet<Email>,
}

impl HasKey for Entity {
    type Key = EntityKey;

    fn key(&self) -> &EntityKey {
        &self.key
    }
}

/// The deployment domain (E6; ARCHITECTURE, Engine > Model: `Deployment`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Deployment {
    /// The deployment revision (H5).
    pub revision: Revision,
    /// The entities.
    #[serde(default, skip_serializing_if = "Keyed::is_empty")]
    pub entities: Keyed<Entity, ByDocumentSize>,
    /// Merged entities' old keys, each resolving to the entity it was merged into (E6).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub aliases: BTreeMap<EntityKey, EntityKey>,
}
