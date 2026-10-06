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
use cairn_assistant::Assistant;
use cairn_auth::Auth;
use cairn_schema::Actor;
use cairn_service::{Call, Service};
use cairn_store::Store;

use crate::endpoints::Endpoint;

/// What every handler holds: the service, auth for its accounts and clock, and the
/// assistant when the host assembled one.
pub(crate) struct Api<S> {
    service: Service<S>,
    auth: Auth<S>,
    assistant: Option<Assistant<S>>,
}

impl<S> Clone for Api<S> {
    fn clone(&self) -> Self {
        Self {
            service: self.service.clone(),
            auth: self.auth.clone(),
            assistant: self.assistant.clone(),
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

/// The API: every endpoint behind `auth`'s layer, with the MCP endpoint at `/mcp` when the
/// service's capabilities offer it (I2), auth's own routes beside them, the request limits,
/// and observability, over `service`. `auth` must share the service's store. No assistant:
/// see [`router_with_assistant`].
pub fn router<S: Store + 'static>(service: Service<S>, auth: &Auth<S>) -> Router {
    router_with_assistant(service, auth, None)
}

/// [`router`], with the assistant's endpoints when the host's root assembled one (I5;
/// ARCHITECTURE, Service layer and composition: absence is a `None` at the root). They sit
/// behind the auth layer and the body and in-flight limits but outside the request
/// duration, since a turn waits on the model provider under its own limits.
///
/// # Panics
///
/// When the service's capabilities say otherwise than `assistant` about the assistant: the
/// capabilities document must say what is served.
pub fn router_with_assistant<S: Store + 'static>(
    service: Service<S>,
    auth: &Auth<S>,
    assistant: Option<Assistant<S>>,
) -> Router {
    use crate::endpoints as at;
    use crate::handlers as handle;
    assert_eq!(
        service.capabilities().assistant,
        assistant.is_some(),
        "the capabilities offer the assistant exactly when one is mounted"
    );
    let api = Api {
        service,
        auth: auth.clone(),
        assistant,
    };
    let mut served: Served<S> = vec![
        (&at::CAPABILITIES, get(handle::capabilities::<S>)),
        (&at::PATCH_JOURNEY, post(handle::patch_journey::<S>)),
        (&at::PATCH_ROUTE, post(handle::patch_route::<S>)),
        (&at::PATCH_DEPLOYMENT, post(handle::patch_deployment::<S>)),
        (&at::JOURNEYS, get(handle::reads::journeys::<S>)),
        (&at::JOURNEY, get(handle::reads::journey::<S>)),
        (&at::DOCUMENT, get(handle::reads::document::<S>)),
        (&at::ROUTES, get(handle::reads::routes::<S>)),
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
    let conversing = api.assistant.is_some().then(|| {
        let mut routes = Router::new();
        for (endpoint, method_router) in handle::assistant::served::<S>() {
            routes = routes.route(endpoint.path, method_router);
        }
        routes.with_state(api.clone())
    });
    let mcp = api.service.capabilities().mcp.then(|| {
        let tools = cairn_mcp::ToolSet::new(api.service.clone(), auth.accounts().clock().clone());
        cairn_mcp::router(tools)
    });
    let routes = routes
        .method_not_allowed_fallback(handle::method_not_allowed)
        .with_state(api);
    // I2: the MCP endpoint, when the host offers it, behind the same auth layer and limits.
    let routes = match mcp {
        Some(mcp) => routes.merge(mcp),
        None => routes,
    };
    // The duration limit is the API's, inside the auth layer: auth's own routes and layer
    // wait on identity providers under their own limit.
    let routes = admission::timed(routes);
    // I5: the assistant's turns, outside the request duration (PRACTICES, Explicit limits:
    // its provider call and turn limits hold them instead).
    let routes = match conversing {
        Some(conversing) => routes.merge(conversing),
        None => routes,
    };
    let app = auth
        .protect(routes)
        .merge(auth.router())
        .fallback(handle::no_such_endpoint);
    admission::limited(app).layer(middleware::from_fn(observe::request))
}
