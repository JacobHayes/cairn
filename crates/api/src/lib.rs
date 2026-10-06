//! The HTTP API (ARCHITECTURE, HTTP API): axum endpoints over the service, behind the auth
//! layer, for every operation the service offers (I1). A rejected patch answers its
//! rejection unchanged (A15); every request is held to the request limits and observed
//! (ARCHITECTURE, Observability).
//!
//! [`router`] is the whole API as an `axum::Router`, so the binary and a testbed serve it on
//! whatever runtime they build (DECISIONS.md, 1.3). A TCP server records peers, as the auth
//! layer needs: `axum::serve(listener, router.into_make_service_with_connect_info::<
//! SocketAddr>())`.

#![forbid(unsafe_code)]

pub mod admission;
pub mod client;
pub mod endpoints;
pub mod error;
pub mod extract;
mod handlers;
pub mod limits;
pub mod observe;
pub mod openapi;
pub mod query;
pub mod stream;
pub mod wire;

use axum::Router;
use axum::middleware;
use axum::routing::{MethodRouter, delete, get, post};
use cairn_auth::Auth;
use cairn_schema::Actor;
use cairn_service::{Call, Service};
use cairn_store::Store;

use crate::endpoints::Endpoint;

/// What every handler holds: the service, and auth for its accounts and clock.
pub(crate) struct Api<S> {
    service: Service<S>,
    auth: Auth<S>,
}

impl<S> Clone for Api<S> {
    fn clone(&self) -> Self {
        Self {
            service: self.service.clone(),
            auth: self.auth.clone(),
        }
    }
}

impl<S: Store + 'static> Api<S> {
    /// The call a request makes: its actor, at the deployment clock's now, read once.
    fn call(&self, actor: Actor) -> Call {
        Call {
            actor,
            now: self.auth.accounts().clock().now(),
        }
    }
}

/// Endpoints and the handlers that serve them.
type Served<S> = Vec<(&'static Endpoint, MethodRouter<Api<S>>)>;

/// The API: every endpoint behind `auth`'s layer, auth's own routes beside them, the request
/// limits, and observability, over `service`. `auth` must share the service's store.
pub fn router<S: Store + 'static>(service: Service<S>, auth: &Auth<S>) -> Router {
    use crate::endpoints as at;
    use crate::handlers as handle;
    let api = Api {
        service,
        auth: auth.clone(),
    };
    let mut served: Served<S> = vec![
        (&at::CAPABILITIES, get(handle::capabilities::<S>)),
        (&at::PATCH_JOURNEY, post(handle::patch_journey::<S>)),
        (&at::PATCH_ROUTE, post(handle::patch_route::<S>)),
        (&at::PATCH_DEPLOYMENT, post(handle::patch_deployment::<S>)),
        (&at::JOURNEYS, get(handle::reads::journeys::<S>)),
        (&at::JOURNEY, get(handle::reads::journey::<S>)),
        (&at::DOCUMENT, get(handle::reads::document::<S>)),
        (&at::ROUTE, get(handle::reads::route::<S>)),
        (&at::ROUTE_VERSIONS, get(handle::reads::route_versions::<S>)),
        (&at::ROUTE_VERSION, get(handle::reads::route_version::<S>)),
        (&at::DEPLOYMENT, get(handle::reads::deployment::<S>)),
        (&at::ENTITY, get(handle::reads::entity::<S>)),
        (&at::SEARCH, get(handle::reads::search::<S>)),
        (&at::EVENTS, get(handle::reads::events::<S>)),
        (&at::STREAM, get(stream::stream::<S>)),
        (&at::VIEWER, get(handle::users::viewer::<S>)),
        (&at::TOKENS, get(handle::users::tokens::<S>)),
        (&at::MINT_TOKEN, post(handle::users::mint_token::<S>)),
        (&at::REVOKE_TOKEN, delete(handle::users::revoke_token::<S>)),
    ];
    served.extend(handle::projections::served::<S>());
    served.extend(handle::proposals::served::<S>());
    served.extend(handle::bulk::served::<S>());
    let mut routes = Router::new();
    for (endpoint, method_router) in served {
        routes = routes.route(endpoint.path, method_router);
    }
    let routes = routes
        .method_not_allowed_fallback(handle::method_not_allowed)
        .with_state(api);
    // The duration limit is the API's, inside the auth layer: auth's own routes and layer
    // wait on identity providers under their own limit.
    let routes = admission::timed(routes);
    let app = auth
        .protect(routes)
        .merge(auth.router())
        .fallback(handle::no_such_endpoint);
    admission::limited(app).layer(middleware::from_fn(observe::request))
}
