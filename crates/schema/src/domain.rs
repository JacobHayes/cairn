//! The patch domains (A17; ARCHITECTURE, Terms: Domain): a journey, a route with its draft,
//! and the deployment, each loaded and revision-checked as one unit, plus the route
//! versions a route publishes, which are immutable graphs loaded one at a time.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::collections::{
    BoundedSet, EmailCountPerIdentity, HasKey, IdentityCountPerDeployment, Keyed,
};
use crate::graph::Graph;
use crate::id::{IdentityKey, JourneyId, RouteId};
use crate::limits::{Limit, LimitExceeded};
use crate::number::{Revision, VersionNumber};
use crate::text::{Email, Markdown, Title};
use jiff::Timestamp;
use jiff::civil::Date;

/// A patch domain (A17).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Domain {
    /// A journey.
    Journey(JourneyId),
    /// A route, including its draft.
    Route(RouteId),
    /// The deployment: its identities and aliases.
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

/// What a route is for (PRD glossary, Route): fixed when it is created.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RouteKind {
    /// A process template: journeys start from it.
    #[default]
    Process,
    /// A reusable piece of a process: it is only inserted, never started (A21).
    Segment,
}

impl RouteKind {
    /// True for the default kind, which a document leaves out.
    #[must_use]
    pub fn is_process(&self) -> bool {
        *self == RouteKind::Process
    }
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
    /// Process or segment (A21); routes stored before segments existed are processes.
    #[serde(default, skip_serializing_if = "RouteKind::is_process")]
    pub kind: RouteKind,
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

/// A person or team journeys refer to (PRD glossary, Identity), deployment-scoped.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    /// The identity's key.
    pub key: IdentityKey,
    /// Its name.
    pub name: Title,
    /// Its emails, each held by at most one identity (H3), at most
    /// `email_count_per_identity_max`. A merge joins two identities' emails, so the engine checks
    /// the joined set too.
    #[serde(
        default,
        skip_serializing_if = "BTreeSet::is_empty",
        deserialize_with = "identity_emails"
    )]
    #[schemars(with = "BoundedSet<Email, EmailCountPerIdentity>")]
    pub emails: BTreeSet<Email>,
}

/// An identity's emails as written: each once, at most `email_count_per_identity_max`.
fn identity_emails<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeSet<Email>, D::Error> {
    BoundedSet::<Email, EmailCountPerIdentity>::deserialize(deserializer).map(BoundedSet::into_set)
}

impl HasKey for Identity {
    type Key = IdentityKey;

    fn key(&self) -> &IdentityKey {
        &self.key
    }
}

/// The deployment domain (E6; ARCHITECTURE, Engine > Model: `Deployment`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Deployment {
    /// The deployment revision (H5).
    pub revision: Revision,
    /// The identities, at most `identity_count_per_deployment_max`.
    #[serde(default, skip_serializing_if = "Keyed::is_empty")]
    pub identities: Keyed<Identity, IdentityCountPerDeployment>,
    /// Merged identities' old keys, each resolving to the identity it was merged into (E6), at
    /// most `alias_count_per_identity_max` resolving to one identity.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "deployment_aliases"
    )]
    pub aliases: BTreeMap<IdentityKey, IdentityKey>,
}

impl Deployment {
    /// Each identity, in key order, that more than `alias_count_per_identity_max` aliases resolve
    /// to, with its alias count. A merge moves the merged identity's aliases to the survivor, so
    /// the engine checks this after one as the parse checks it on read.
    #[must_use]
    pub fn alias_counts_exceeded(&self) -> Vec<(&IdentityKey, LimitExceeded)> {
        alias_counts_exceeded(&self.aliases)
    }
}

fn alias_counts_exceeded(
    aliases: &BTreeMap<IdentityKey, IdentityKey>,
) -> Vec<(&IdentityKey, LimitExceeded)> {
    let mut counts: BTreeMap<&IdentityKey, usize> = BTreeMap::new();
    for target in aliases.values() {
        *counts.entry(target).or_default() += 1;
    }
    counts
        .into_iter()
        .filter_map(|(target, count)| {
            Limit::AliasCountPerIdentity
                .check(count)
                .err()
                .map(|exceeded| (target, exceeded))
        })
        .collect()
}

/// The deployment's aliases as written: each alias once, and at most
/// `alias_count_per_identity_max` resolving to one identity.
fn deployment_aliases<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<IdentityKey, IdentityKey>, D::Error> {
    let aliases = crate::serde_util::unique_map(deserializer)?;
    if let Some((target, exceeded)) = alias_counts_exceeded(&aliases).first() {
        return Err(serde::de::Error::custom(format!(
            "aliases of {target}: {exceeded}"
        )));
    }
    Ok(aliases)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_and_past<T: serde::de::DeserializeOwned + fmt::Debug>(
        limit: Limit,
        document: impl Fn(u32) -> serde_json::Value,
    ) {
        assert!(
            serde_json::from_value::<T>(document(limit.max())).is_ok(),
            "{limit}"
        );
        let past = serde_json::from_value::<T>(document(limit.max() + 1)).unwrap_err();
        assert!(past.to_string().contains(limit.name()), "{limit}: {past}");
    }

    #[test]
    fn an_identity_holds_its_email_limit() {
        at_and_past::<Identity>(Limit::EmailCountPerIdentity, |count| {
            let emails: Vec<String> = (0..count).map(|n| format!("p{n}@example.org")).collect();
            serde_json::json!({"key": "who_person", "name": "Person", "emails": emails})
        });
    }

    #[test]
    fn a_deployment_holds_its_identity_limit() {
        at_and_past::<Deployment>(Limit::IdentityCountPerDeployment, |count| {
            let identities: Vec<_> = (0..count)
                .map(|n| serde_json::json!({"key": format!("who_{n}"), "name": "Someone"}))
                .collect();
            serde_json::json!({"revision": 1, "identities": identities})
        });
    }

    #[test]
    fn a_deployment_holds_its_alias_limit_per_identity() {
        at_and_past::<Deployment>(Limit::AliasCountPerIdentity, |count| {
            let aliases: serde_json::Map<_, _> = (0..count)
                .map(|n| (format!("who_old_{n}"), serde_json::json!("who_survivor")))
                .collect();
            serde_json::json!({"revision": 1, "aliases": aliases})
        });
    }

    #[test]
    fn aliases_spread_over_identities_are_each_within_the_limit() {
        let aliases: serde_json::Map<_, _> = (0..2 * ALIAS_LIMIT)
            .map(|n| {
                (
                    format!("who_old_{n}"),
                    serde_json::json!(format!("who_{}", n % 2)),
                )
            })
            .collect();
        let document = serde_json::json!({"revision": 1, "aliases": aliases});
        assert!(serde_json::from_value::<Deployment>(document).is_ok());
    }

    const ALIAS_LIMIT: u32 = crate::limits::ALIAS_COUNT_PER_IDENTITY_MAX;
}
