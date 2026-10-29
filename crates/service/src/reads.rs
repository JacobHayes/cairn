//! The store-backed reads (ARCHITECTURE, Store trait): whole documents by id, the journey
//! index (C16), the route index, route detail (C17), text search across journeys, events (J5), identity
//! resolution (E6), and subscriptions (H6). None depends on who asks: one deployment is one
//! trust boundary (H4), so reads take no call. Derived reads and projections, which depend on
//! the caller's today and identities, are in `projections`.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    Deployment, GraphId, IdentityKey, Journey, JourneyId, Revision, Route, RouteHeader, RouteId,
    RouteVersion, VersionNumber,
};
use cairn_store::{
    Document, EventQuery, JourneyMatches, JourneyQuery, JourneySummary, LoadTarget, LoggedEvent,
    Page, PageSize, RouteDetail, SearchQuery, Subscription, Watch,
};

use crate::{Service, ServiceError};

/// A route in the route index: its fields, revision, latest version, and whether a draft is
/// open (A11).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteSummary {
    /// The route's fields.
    pub header: RouteHeader,
    /// Its revision.
    pub revision: Revision,
    /// Its latest published version, if any.
    pub latest_version: Option<VersionNumber>,
    /// Whether it has an open draft.
    pub draft_open: bool,
}

/// E6: where a route refers to an identity: which of its graphs do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteReference {
    /// The route's fields.
    pub header: RouteHeader,
    /// Whether its draft refers to the identity.
    pub draft: bool,
    /// The published versions that refer to it.
    pub versions: Vec<VersionNumber>,
}

/// E6: everything that refers to an identity, directly or through an alias: the journeys, and
/// the routes whose draft or published versions do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityReferences {
    /// The journeys, in id order, whatever their status.
    pub journeys: Vec<JourneySummary>,
    /// The routes, in id order.
    pub routes: Vec<RouteReference>,
}

impl Service {
    /// E6, E7: every journey referring to any of `identities`, directly or through an alias, at
    /// its current revision: the set a merge of them names, which the store checks at commit.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn merge_revisions(
        &self,
        identities: BTreeSet<IdentityKey>,
    ) -> Result<BTreeMap<JourneyId, Revision>, ServiceError> {
        let journeys = self.journeys_referring_to(identities).await?;
        Ok(journeys
            .into_iter()
            .map(|summary| (summary.id, summary.revision))
            .collect())
    }

    /// The index read behind both: every journey referring to any of `identities`, a page at
    /// a time.
    async fn journeys_referring_to(
        &self,
        identities: BTreeSet<IdentityKey>,
    ) -> Result<Vec<JourneySummary>, ServiceError> {
        let mut journeys = Vec::new();
        let mut query = JourneyQuery {
            referencing: Some(identities),
            size: PageSize::MAX,
            ..JourneyQuery::default()
        };
        loop {
            let page = self.store.journeys(&query).await?;
            journeys.extend(page.items);
            match page.next {
                Some(after) => query.after = Some(after),
                None => return Ok(journeys),
            }
        }
    }

    /// E6: the journeys and routes referring to the identity `identity`, or none when the
    /// deployment has no such identity. Authoritative: the store's exact references over every
    /// graph (journeys, route drafts and published versions), the same set a delete is
    /// checked against. Cost: one reference query, then a journey-index read per page of the
    /// journeys and one route load per route.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn identity_references(
        &self,
        identity: &IdentityKey,
    ) -> Result<Option<IdentityReferences>, ServiceError> {
        if self.deployment().await?.identities.get(identity).is_none() {
            return Ok(None);
        }
        let graphs = self
            .store
            .graphs_referencing(&BTreeSet::from([identity.clone()]))
            .await?;
        let journeys = self
            .journeys_referring_to(BTreeSet::from([identity.clone()]))
            .await?;
        let mut drafts = BTreeSet::new();
        let mut versions: BTreeMap<RouteId, Vec<VersionNumber>> = BTreeMap::new();
        for graph in &graphs {
            match graph {
                GraphId::Journey(_) => {}
                GraphId::RouteDraft(route) => {
                    drafts.insert(route.clone());
                }
                GraphId::RouteVersion { route, version } => {
                    versions.entry(route.clone()).or_default().push(*version);
                }
            }
        }
        let routes: BTreeSet<&RouteId> = drafts.iter().chain(versions.keys()).collect();
        let mut referring = Vec::with_capacity(routes.len());
        for id in routes {
            // A route the references name always loads: routes are never deleted (A19).
            if let Some(route) = self.route(id).await? {
                referring.push(RouteReference {
                    header: route.header,
                    draft: drafts.contains(id),
                    versions: versions.get(id).cloned().unwrap_or_default(),
                });
            }
        }
        Ok(Some(IdentityReferences {
            journeys,
            routes: referring,
        }))
    }

    /// I2: the route index, in id order, paged: the routes after `after`, at most `size`. Cost: the store's revisions of every domain (a row each),
    /// then one route load per route passed over or on the page.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn routes(
        &self,
        after: Option<&RouteId>,
        size: PageSize,
    ) -> Result<Page<RouteSummary, RouteId>, ServiceError> {
        let revisions = self.store.revisions().await?;
        let ids = revisions
            .routes
            .into_keys()
            .filter(|id| after.is_none_or(|after| id > after));
        let mut items = Vec::with_capacity(size.len().saturating_add(1));
        for id in ids {
            if items.len() > size.len() {
                break;
            }
            // A route the revisions name always loads: routes are never deleted (A19).
            if let Some(route) = self.route(&id).await? {
                items.push(RouteSummary {
                    latest_version: route.versions.last().copied(),
                    draft_open: route.draft.is_some(),
                    revision: route.revision,
                    header: route.header,
                });
            }
        }
        Ok(Page::cut(items, size, |summary| summary.header.id.clone()))
    }

    /// A journey with its graph and state (G1, B1), or none.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn journey(&self, id: &JourneyId) -> Result<Option<Journey>, ServiceError> {
        let loaded = self.store.load(&LoadTarget::Journey(id.clone())).await?;
        Ok(loaded.map(|document| match document {
            Document::Journey(journey) => journey,
            other => unreachable!("a journey load returned {other:?}"),
        }))
    }

    /// A route with its draft (A11), or none.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn route(&self, id: &RouteId) -> Result<Option<Route>, ServiceError> {
        let loaded = self.store.load(&LoadTarget::Route(id.clone())).await?;
        Ok(loaded.map(|document| match document {
            Document::Route(route) => route,
            other => unreachable!("a route load returned {other:?}"),
        }))
    }

    /// One published route version (A11), or none.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn route_version(
        &self,
        route: &RouteId,
        version: VersionNumber,
    ) -> Result<Option<RouteVersion>, ServiceError> {
        let target = LoadTarget::RouteVersion {
            route: route.clone(),
            version,
        };
        let loaded = self.store.load(&target).await?;
        Ok(loaded.map(|document| match document {
            Document::RouteVersion(version) => version,
            other => unreachable!("a version load returned {other:?}"),
        }))
    }

    /// The deployment: identities, aliases, and its revision (E6). It always exists.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn deployment(&self) -> Result<Deployment, ServiceError> {
        match self.store.load(&LoadTarget::Deployment).await? {
            Some(Document::Deployment(deployment)) => Ok(deployment),
            other => unreachable!("the deployment always loads, not {other:?}"),
        }
    }

    /// C16: the journey index, filtered and paged.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn journeys(
        &self,
        query: &JourneyQuery,
    ) -> Result<Page<JourneySummary, JourneyId>, ServiceError> {
        Ok(self.store.journeys(query).await?)
    }

    /// C17: a route's published versions with the journeys on each, or none.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn route_detail(&self, route: &RouteId) -> Result<Option<RouteDetail>, ServiceError> {
        Ok(self.store.route_detail(route).await?)
    }

    /// Text search across journeys over titles, descriptions, notes, and resources, paged.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn search(
        &self,
        query: &SearchQuery,
    ) -> Result<Page<JourneyMatches, JourneyId>, ServiceError> {
        Ok(self.store.search(query).await?)
    }

    /// J5: events by journey, node, user, type, patch, and time, in commit order, paged.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn events(&self, query: &EventQuery) -> Result<Page<LoggedEvent, u64>, ServiceError> {
        Ok(self.store.events(query).await?)
    }

    /// H6: a subscription to what `watching` names. It is registered with the notifier
    /// before the store's current revisions are read and seeded into it, so a commit landing
    /// in between is announced either way and never missed; its first take hands the
    /// current revisions over at once.
    ///
    /// # Errors
    ///
    /// When the process is at its subscriber limit, or the store fails.
    pub async fn subscribe(&self, watching: BTreeSet<Watch>) -> Result<Subscription, ServiceError> {
        let subscription = self.notifier.subscribe(watching)?;
        let current = self.store.revisions().await?;
        subscription.seed(&current);
        Ok(subscription)
    }
}
