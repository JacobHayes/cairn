//! The store-backed reads (ARCHITECTURE, Store trait): whole documents by id, the journey
//! index (C16), route detail (C17), text search across journeys, events (J5), entity
//! resolution (E6), and subscriptions (H6). None depends on who asks: one deployment is one
//! trust boundary (H4), so reads take no call. Derived reads and projections are 4.8's.

use std::collections::BTreeSet;

use cairn_schema::{
    Deployment, Entity, EntityKey, Journey, JourneyId, Route, RouteId, RouteVersion, VersionNumber,
};
use cairn_store::{
    Document, EventQuery, JourneyMatches, JourneyQuery, JourneySummary, LoadTarget, LoggedEvent,
    Page, RouteDetail, SearchQuery, Store, Subscription, Watch,
};

use crate::{Service, ServiceError};

impl<S: Store> Service<S> {
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
