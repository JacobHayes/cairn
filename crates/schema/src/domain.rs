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
use crate::node::IdentitySet;
use crate::number::{Revision, VersionNumber};
use crate::text::{Email, Markdown, Title};
use jiff::Timestamp;
use jiff::civil::Date;

// PRD: A17.
/// A patch domain.
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

// PRD: A19.
/// A journey or route a hard delete removed: its id is never reused, so a late create cannot
/// bring it back.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum DeletedId {
    /// A journey.
    Journey(JourneyId),
    /// A route, with its draft and every version.
    Route(RouteId),
}

impl DeletedId {
    /// The domain the id named.
    #[must_use]
    pub fn domain(&self) -> Domain {
        match self {
            DeletedId::Journey(id) => Domain::Journey(id.clone()),
            DeletedId::Route(id) => Domain::Route(id.clone()),
        }
    }

    /// The id a domain leaves when it is deleted; none for the deployment, which never is.
    #[must_use]
    pub fn of(domain: &Domain) -> Option<DeletedId> {
        match domain {
            Domain::Journey(id) => Some(DeletedId::Journey(id.clone())),
            Domain::Route(id) => Some(DeletedId::Route(id.clone())),
            Domain::Deployment => None,
        }
    }
}

impl fmt::Display for DeletedId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.domain().fmt(formatter)
    }
}

// ARCHITECTURE: Schema outline: `graphs`.
/// Which graph a stored record belongs to.
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

// PRD: glossary, Lineage.
/// The route version a journey was made from.
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

// PRD: B11.
/// A journey's status.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum JourneyStatus {
    /// In progress.
    Active,
    /// Marked complete; still patchable, out of "mine" and cross-journey lists.
    Completed,
    /// Archived: accepts only a move to active or completed, or hard deletion.
    Archived,
}

impl JourneyStatus {
    /// Every status, in lifecycle order.
    pub const ALL: [JourneyStatus; 3] = [
        JourneyStatus::Active,
        JourneyStatus::Completed,
        JourneyStatus::Archived,
    ];

    /// The status as written.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            JourneyStatus::Active => "active",
            JourneyStatus::Completed => "completed",
            JourneyStatus::Archived => "archived",
        }
    }

    // PRD: B11.
    /// The statuses a journey in this one may move to: active and completed both ways,
    /// either archived, and an archived journey to active or completed.
    #[must_use]
    pub const fn moves(self) -> &'static [JourneyStatus] {
        match self {
            JourneyStatus::Active => &[JourneyStatus::Completed, JourneyStatus::Archived],
            JourneyStatus::Completed => &[JourneyStatus::Active, JourneyStatus::Archived],
            JourneyStatus::Archived => &[JourneyStatus::Active, JourneyStatus::Completed],
        }
    }

    // PRD: B11, A19.
    /// Whether a journey in this status accepts changes beyond its status moves and hard
    /// deletion: an archived journey does not.
    #[must_use]
    pub const fn accepts_changes(self) -> bool {
        match self {
            JourneyStatus::Active | JourneyStatus::Completed => true,
            JourneyStatus::Archived => false,
        }
    }

    // PRD: B11.
    /// Whether a journey in this status holds current work: an active one does; a completed
    /// or archived journey leaves "mine" and the cross-journey lists.
    #[must_use]
    pub const fn holds_current_work(self) -> bool {
        match self {
            JourneyStatus::Active => true,
            JourneyStatus::Completed | JourneyStatus::Archived => false,
        }
    }
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
    // PRD: B1, C16.
    /// Who is responsible for the journey as a whole: zero or more identities.
    #[serde(default, skip_serializing_if = "IdentitySet::is_empty")]
    pub owners: IdentitySet,
    /// Its status.
    pub status: JourneyStatus,
    /// The route version it follows, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lineage: Option<Lineage>,
    /// When it was created.
    pub created_at: Timestamp,
    // PRD: A8, A9.
    /// The deployment-local calendar day it was created, the `created_at` date rules
    /// measure from. Stored beside the timestamp so the engine never converts time
    /// zones.
    pub created_on: Date,
}

/// A journey domain: its fields, revision, and graph with state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Journey {
    /// The journey's own fields.
    pub header: JourneyHeader,
    // PRD: H5.
    /// The journey revision.
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
    // PRD: A19.
    /// Hidden from new-journey creation.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub retired: bool,
}

// PRD: A11.
/// A route's single mutable draft.
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
    // PRD: H5.
    /// The route revision, covering the draft.
    pub revision: Revision,
    /// The published versions.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub versions: BTreeSet<VersionNumber>,
    /// The draft, if one is open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<RouteDraft>,
}

// PRD: A11.
/// An immutable published route version.
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

// PRD: glossary, Identity.
/// A person or team journeys refer to, deployment-scoped.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    /// The identity's key.
    pub key: IdentityKey,
    // PRD: H3.
    /// Its name, if it has one. An identity has a name, an email, or both, so one a
    /// sign-in created from an account that gave no name is shown by its first email until
    /// its person is asked for one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<Title>,
    // PRD: H3.
    /// Its emails, each held by at most one identity, at most
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

impl Identity {
    /// How the identity is shown (H3): its name, else its first email. An identity has one of
    /// them, so this is never empty.
    #[must_use]
    pub fn display(&self) -> &str {
        match (&self.name, self.emails.first()) {
            (Some(name), _) => name.as_str(),
            (None, Some(email)) => email.as_str(),
            (None, None) => unreachable!("an identity has a name, an email, or both"),
        }
    }
}

impl HasKey for Identity {
    type Key = IdentityKey;

    fn key(&self) -> &IdentityKey {
        &self.key
    }
}

// PRD: E6.
// ARCHITECTURE: Engine > Model: `Deployment`.
/// The deployment domain.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Deployment {
    // PRD: H5.
    /// The deployment revision.
    pub revision: Revision,
    /// The identities, at most `identity_count_per_deployment_max`.
    #[serde(default, skip_serializing_if = "Keyed::is_empty")]
    pub identities: Keyed<Identity, IdentityCountPerDeployment>,
    // PRD: E6.
    /// Merged identities' old keys, each resolving to the identity it was merged into, at
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
