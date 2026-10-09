//! The store-backed reads (ARCHITECTURE, Store trait): whole documents by id, the journey
//! index (C16), the route index, route detail (C17), text search across journeys, events (J5), entity
//! resolution (E6), and subscriptions (H6). None depends on who asks: one deployment is one
//! trust boundary (H4), so reads take no call. Derived reads and projections, which depend on
//! the caller's today and entities, are in `projections`.

use std::collections::BTreeSet;

use cairn_schema::{
    Deployment, Entity, EntityKey, Journey, JourneyId, Revision, Route, RouteHeader, RouteId,
    RouteKind, RouteVersion, VersionNumber,
};
use cairn_store::{
    Document, EventQuery, JourneyMatches, JourneyQuery, JourneySummary, LoadTarget, LoggedEvent,
    Page, PageSize, RouteDetail, SearchQuery, Store, Subscription, Watch,
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

impl<S: Store> Service<S> {
    /// I2: the route index, in id order, paged: the routes of `kind` (every kind when none)
    /// after `after`, at most `size`. Cost: the store's revisions of every domain (a row each),
    /// then one route load per route passed over or on the page.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn routes(
        &self,
        kind: Option<RouteKind>,
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
            if let Some(route) = self.route(&id).await?
                && kind.is_none_or(|kind| route.header.kind == kind)
            {
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

    /// The deployment: entities, aliases, and its revision (E6). It always exists.
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

    /// E6: the entity a key names, through an alias when it was merged away, or none.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn entity(&self, key: &EntityKey) -> Result<Option<Entity>, ServiceError> {
        Ok(self.store.resolve_entity(key).await?)
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
