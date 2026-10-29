//! The in-browser root's reads for the screens around a journey: the journey
//! index filtered as `GET /api/journeys` filters it (C16), the route index and route detail (C17),
//! route files exported and imported through the service (A13), and the caller with their
//! logins and identities (H3). Each answers the API's JSON for the same request, so the
//! app's one data layer reads either host.

use std::collections::BTreeSet;

use cairn_schema::{
    IdentityKey, JourneyId, JourneyStatus, RouteId, RouteKind, UserId, VersionNumber,
};
use cairn_service::ReadError;
use cairn_store::{JourneyQuery, PageSize};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::error::{HostError, json, read};
use crate::now_or_never;
use crate::root::{BrowserRoot, failed};
use cairn_service::wire::{
    Caller, IdentityReferences, JourneyPage, PatchAnswer, RouteDetail, RouteImport, RoutePage,
    UserName,
};

/// `GET /api/journeys`'s query (C16) as JSON: the statuses, the lineage route and version, the
/// identities referred to (E6), whether an upgrade is available, and the page.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JourneyIndexQuery {
    /// Journeys in any of these statuses; none for any status.
    #[serde(default)]
    pub status: BTreeSet<JourneyStatus>,
    /// Journeys following this route.
    #[serde(default)]
    pub route: Option<RouteId>,
    /// Journeys on this version of their route.
    #[serde(default)]
    pub version: Option<VersionNumber>,
    /// Journeys referring to any of these identities, directly or through an alias.
    #[serde(default)]
    pub referencing: BTreeSet<IdentityKey>,
    /// Journeys owned by any of these identities, directly or through an alias.
    #[serde(default)]
    pub owner: BTreeSet<IdentityKey>,
    /// Journeys whose route has, or has not, published a version newer than theirs.
    #[serde(default)]
    pub upgrade_available: Option<bool>,
    /// The page starts after this journey.
    #[serde(default)]
    pub after: Option<JourneyId>,
    /// The page's size, cut to the page limit; the limit when none.
    #[serde(default)]
    pub size: Option<u32>,
}

impl JourneyIndexQuery {
    /// The store's query, as the API reads its parameters.
    fn store_query(&self) -> JourneyQuery {
        JourneyQuery::from_parameters(
            self.status.clone(),
            self.route.clone(),
            self.version,
            self.referencing.clone(),
            self.upgrade_available,
            self.after.clone(),
            self.size,
        )
        .owned_by(self.owner.clone())
    }
}

impl BrowserRoot {
    /// C16: the journey index `query` asks for, as `GET /api/journeys` answers it.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub fn journey_index(&self, query: &JourneyIndexQuery) -> Result<JourneyPage, HostError> {
        let page = now_or_never(self.service().journeys(&query.store_query())).map_err(failed)?;
        Ok(page.into())
    }

    /// E6: what refers to identity `identity`, as
    /// `GET /api/deployment/identities/{key}/references` answers it.
    ///
    /// # Errors
    ///
    /// [`HostError::Missing`] when there is no such identity; when the store fails.
    pub fn references_of(&self, identity: &IdentityKey) -> Result<IdentityReferences, HostError> {
        let Some(found) =
            now_or_never(self.service().identity_references(identity)).map_err(failed)?
        else {
            return Err(HostError::Missing {
                message: format!("no identity {identity}"),
            });
        };
        Ok(found.into())
    }

    /// The route index: the routes and segments of `kind` (both when none), in id order, as
    /// `GET /api/routes` answers its first page at the page limit.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub fn route_page(
        &self,
        kind: Option<RouteKind>,
        after: Option<&RouteId>,
    ) -> Result<RoutePage, HostError> {
        let page =
            now_or_never(self.service().routes(kind, after, PageSize::MAX)).map_err(failed)?;
        Ok(page.into())
    }

    /// C17: route `id`'s versions with the journeys on each.
    ///
    /// # Errors
    ///
    /// [`HostError::Missing`] when there is no such route; when the store fails.
    pub fn detail_of(&self, id: &RouteId) -> Result<RouteDetail, HostError> {
        let Some(detail) = now_or_never(self.service().route_detail(id)).map_err(failed)? else {
            return Err(HostError::Missing {
                message: format!("no route {id}"),
            });
        };
        Ok(detail.into())
    }

    /// The caller: the local user, its login, and the identities its emails name (H3).
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub fn caller_now(&self, now: &str) -> Result<Caller, HostError> {
        let call = self.call(now)?;
        let caller = now_or_never(self.service().caller(&call)).map_err(failed)?;
        Ok(Caller::of(caller, None))
    }

    /// H3: the names of `users`; one that does not exist is left out.
    ///
    /// # Errors
    ///
    /// [`HostError::Refused`] for more users than a page holds; when the store fails.
    pub fn user_names_now(&self, users: &[UserId]) -> Result<Vec<UserName>, HostError> {
        let names =
            now_or_never(self.service().user_names(users)).map_err(|error| match error {
                ReadError::TooManyUsers { .. } => HostError::Refused {
                    message: error.to_string(),
                },
                error => failed(error),
            })?;
        Ok(names.into_iter().map(Into::into).collect())
    }

    /// A13: `request`'s file imported as the local user at `now`.
    ///
    /// # Errors
    ///
    /// [`HostError::Rejected`] with every violation of the file or the draft, or for a stale
    /// or reused patch; [`HostError::Failed`] when the store fails.
    pub fn import_file(&self, request: &RouteImport, now: &str) -> Result<PatchAnswer, HostError> {
        let call = self.call(now)?;
        let written = now_or_never(self.service().import_route(
            &call,
            request.patch_id.clone(),
            &request.file,
            request.note.clone(),
        ));
        written
            .map(Into::into)
            .map_err(crate::proposals::write_failed)
    }
}

/// A route id read from JavaScript's text.
fn route_id(route: &str) -> Result<RouteId, HostError> {
    route
        .parse()
        .map_err(|error| HostError::unreadable("route", format!("{error:?}")))
}

#[wasm_bindgen]
impl BrowserRoot {
    /// C16: the journey index `query` (the JSON of a [`JourneyIndexQuery`]) asks for, as
    /// `GET /api/journeys` answers it.
    ///
    /// # Errors
    ///
    /// The JSON of a [`HostError`].
    #[wasm_bindgen(js_name = journeyIndex)]
    pub fn journey_index_json(&self, query: &str) -> Result<String, String> {
        let query: JourneyIndexQuery = read("journey query", query)?;
        Ok(json(&self.journey_index(&query)?))
    }

    /// E6: what refers to identity `identity`, as
    /// `GET /api/deployment/identities/{key}/references` answers it.
    ///
    /// # Errors
    ///
    /// The JSON of a [`HostError`]: no such identity, or an unreadable input.
    #[wasm_bindgen(js_name = identityReferences)]
    pub fn identity_references(&self, identity: &str) -> Result<String, String> {
        let key: IdentityKey = identity
            .parse()
            .map_err(|error| HostError::unreadable("identity", format!("{error:?}")))?;
        Ok(json(&self.references_of(&key)?))
    }

    /// The route index of `kind` (the JSON of an optional kind: `null` for both) from after
    /// `after` (empty for the first page), as `GET /api/routes` answers it.
    ///
    /// # Errors
    ///
    /// The JSON of a [`HostError`].
    pub fn routes(&self, after: &str, kind: &str) -> Result<String, String> {
        let kind: Option<RouteKind> = read("kind", kind)?;
        let after = if after.is_empty() {
            None
        } else {
            Some(route_id(after)?)
        };
        Ok(json(&self.route_page(kind, after.as_ref())?))
    }

    /// C17: route `route`'s versions with the journeys on each, as
    /// `GET /api/routes/{id}/versions` answers it.
    ///
    /// # Errors
    ///
    /// The JSON of a [`HostError`]: no such route, or an unreadable input.
    #[wasm_bindgen(js_name = routeDetail)]
    pub fn route_detail(&self, route: &str) -> Result<String, String> {
        Ok(json(&self.detail_of(&route_id(route)?)?))
    }

    /// A13: version `version` of route `route` as a file, or its draft when `version` is
    /// empty, as `GET /api/routes/{id}/export` answers it.
    ///
    /// # Errors
    ///
    /// The JSON of a [`HostError`]: no such route, version, or draft, or an unreadable input.
    #[wasm_bindgen(js_name = exportFile)]
    pub fn export_file(&self, route: &str, version: &str) -> Result<String, String> {
        let id = route_id(route)?;
        let number: Option<VersionNumber> = if version.is_empty() {
            None
        } else {
            Some(read("version", version)?)
        };
        match now_or_never(self.service().export_route(&id, number)).map_err(failed)? {
            Some(file) => Ok(json(&file)),
            None => Err(HostError::Missing {
                message: format!("route {id} has no such version or draft"),
            }
            .into()),
        }
    }

    /// A13: imports a route file as the local user at `now`: `request` is the JSON of a
    /// [`RouteImport`]; the answer is the JSON of the patch answer, as
    /// `POST /api/routes/{id}/import` answers it.
    ///
    /// # Errors
    ///
    /// The JSON of a [`HostError`], the rejection among them.
    #[wasm_bindgen(js_name = importFile)]
    pub fn import_file_json(&self, request: &str, now: &str) -> Result<String, String> {
        let request: RouteImport = read("route import", request)?;
        Ok(json(&self.import_file(&request, now)?))
    }

    /// The caller at `now`, as `GET /api/users/me` answers it (H3).
    ///
    /// # Errors
    ///
    /// The JSON of a [`HostError`].
    pub fn caller(&self, now: &str) -> Result<String, String> {
        Ok(json(&self.caller_now(now)?))
    }

    /// The names of the users in `users`, a JSON array of ids, as `GET /api/users` answers it
    /// (H3).
    ///
    /// # Errors
    ///
    /// The JSON of a [`HostError`].
    #[wasm_bindgen(js_name = userNames)]
    pub fn user_names_json(&self, users: &str) -> Result<String, String> {
        let users: Vec<UserId> = read("users", users)?;
        Ok(json(&self.user_names_now(&users)?))
    }
}
