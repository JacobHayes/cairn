//! What the API tests share: a server on loopback over a memory store, run on the test's
//! own runtime; a clock the test sets; clients signed in as one of the team; and the
//! fixtures' route and scenario patches.

#![allow(dead_code)]

pub mod mcp;

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use async_trait::async_trait;
use axum::http::Method;
use cairn_api::client::{Reply, Transport};
use cairn_auth::{
    Accounts, Auth, AuthProvider, Clock, GcpIapConfig, GcpIapProvider, GoogleEndpoints, Listener,
    OAuthConfig, OAuthServer, Presented, RelayClient, Verdict,
};
use cairn_engine::from_file;
use cairn_schema::{
    DraftSource, Login, Mutation, Mutations, Patch, PatchTarget, RankConstants, Revision,
    RouteFile, Scenario, SequentialKeys, Slug, Timestamp, from_yaml,
};
use cairn_service::{AuthKind, AuthMethod, Capabilities, DeploymentSettings, Parts, Service};
use cairn_store::{Faults, InProcessNotifier, MemoryStore};
use jiff::tz::{Offset, TimeZone};
use serde::de::DeserializeOwned;
use url::Url;

use crate::auth::support::iap::{self, AUDIENCE, FakeIap, KeySet};
use crate::auth::support::issuer::Issuer;

/// The team the test sign-in provider knows, with each one's verified email.
pub const TEAM: [(&str, &str); 2] = [("ann", "ana@example.org"), ("bob", "bob@example.org")];

/// A clock the test sets.
#[derive(Clone, Default)]
pub struct TestClock(Arc<AtomicI64>);

impl TestClock {
    pub fn clock(&self) -> Clock {
        let seconds = Arc::clone(&self.0);
        Clock::from_fn(move || Timestamp::from_second(seconds.load(Ordering::SeqCst)).unwrap())
    }

    /// Sets it to `at`, an RFC 3339 time.
    pub fn set(&self, at: &str) {
        let at: Timestamp = at.parse().unwrap();
        self.0.store(at.as_second(), Ordering::SeqCst);
    }

    /// What it reads now.
    pub fn now(&self) -> Timestamp {
        Timestamp::from_second(self.0.load(Ordering::SeqCst)).unwrap()
    }

    pub fn set_to(&self, at: Timestamp) {
        self.0.store(at.as_second(), Ordering::SeqCst);
    }
}

/// A sign-in provider for the tests, standing for any provider that vouches for verified
/// emails: `Bearer team-<name>` signs in as that member of [`TEAM`].
struct TeamProvider {
    name: Slug,
}

#[async_trait]
impl AuthProvider for TeamProvider {
    fn name(&self) -> &Slug {
        &self.name
    }

    async fn authenticate(&self, request: Presented<'_>) -> Verdict {
        let member = request
            .bearer()
            .and_then(|token| token.strip_prefix("team-"))
            .and_then(|name| TEAM.iter().find(|(member, _)| *member == name));
        match member {
            None => Verdict::Absent,
            Some((name, email)) => Verdict::Login(Login {
                provider: self.name.clone(),
                subject: name.parse().unwrap(),
                name: Some(name.parse().unwrap()),
                verified_emails: BTreeSet::from([email.parse().unwrap()]),
            }),
        }
    }

    fn claims_bearer(&self, token: &str) -> bool {
        token.starts_with("team-")
    }
}

/// The Google OAuth client on IAP's programmatic allowlist, through which the relay signs
/// MCP clients in.
pub const GOOGLE_CLIENT: &str = "relay-client.apps.example";
/// That client's secret, which only Cairn and Google see.
pub const GOOGLE_SECRET: &str = "relay-secret-value";

/// The test deployment's time zone: five hours behind UTC all year.
pub fn settings() -> DeploymentSettings {
    let zone = TimeZone::fixed(Offset::constant(-5));
    DeploymentSettings::new("Etc/GMT+5".parse().unwrap(), zone, RankConstants::default())
}

/// A server on loopback and what it stands on.
pub struct World {
    pub service: Service,
    pub store: Arc<MemoryStore>,
    /// The store's faults, for a test that fails the store closed.
    pub faults: Faults,
    pub notifier: Arc<InProcessNotifier>,
    pub clock: TestClock,
    pub address: SocketAddr,
    /// The API as served, for clients that call it in process (the MCP client).
    pub router: axum::Router,
}

impl World {
    /// Starts a server over a fresh memory store on this runtime, its clock at the vendor
    /// evaluation's first step.
    pub async fn start() -> Self {
        Self::begin(true, None, start_clock(), &Front::Team).await
    }

    /// As [`World::start`], for a host whose root assembled the assistant over `provider`
    /// (I5): its capabilities offer it and its endpoints are served.
    pub async fn start_with_assistant(provider: Arc<dyn cairn_assistant::Provider>) -> Self {
        Self::begin(true, Some(provider), start_clock(), &Front::Team).await
    }

    /// As [`World::start`], for a host that does not offer MCP, as the browser host does not
    /// (capability gating: its root serves no MCP endpoint).
    pub async fn start_without_mcp() -> Self {
        Self::begin(false, None, start_clock(), &Front::Team).await
    }

    /// A deployment behind Google Cloud IAP with the relay on (I2): the IAP provider alone,
    /// calling a fake Google, and the server reached only through a fake IAP in front of
    /// it; and the fake Google, for the test to sign in at.
    pub async fn behind_iap() -> (Self, Issuer) {
        let clock = start_clock();
        let google = Issuer::start_google(clock.clock(), GOOGLE_CLIENT, GOOGLE_SECRET).await;
        let key = iap::key("iap", 5);
        let keys = KeySet::start(&[&key]).await;
        let fake = FakeIap {
            google: google.clone(),
            allowlisted: GOOGLE_CLIENT.to_owned(),
            key,
            clock: clock.clock(),
        };
        let endpoints = GoogleEndpoints {
            iap_key_set: keys.url.clone(),
            authorization: google.authorization_url(),
            token: google.token_url(),
        };
        let front = Front::Iap(Box::new((fake, endpoints)));
        (Self::begin(true, None, clock, &front).await, google)
    }

    async fn begin(
        mcp: bool,
        assistant: Option<Arc<dyn cairn_assistant::Provider>>,
        clock: TestClock,
        front: &Front,
    ) -> Self {
        let faults = Faults::default();
        let store = Arc::new(MemoryStore::with_faults(faults.clone()));
        let notifier = Arc::new(InProcessNotifier::new());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let base = format!("http://{address}").parse().unwrap();
        let accounts = Accounts::new(Arc::<MemoryStore>::clone(&store), clock.clock());
        let (providers, methods) = front.providers(&accounts, &base, clock.clock());
        let auth = Auth::new(accounts, Listener::Tcp(address), &base, providers);
        let service = Service::new(Parts {
            store: Arc::<MemoryStore>::clone(&store),
            notifier: notifier.clone(),
            settings: settings(),
            capabilities: Capabilities {
                mcp,
                ..Capabilities::server(methods, assistant.is_some())
            },
        });
        let assistant = assistant.map(|provider| {
            let store = Arc::<MemoryStore>::clone(&store);
            cairn_assistant::Assistant::new(service.clone(), store, clock.clock(), provider)
        });
        let app = front.in_front_of(cairn_api::router_with_assistant(
            service.clone(),
            &auth,
            assistant,
        ));
        let router = app.clone();
        tokio::spawn(async move {
            let app = app.into_make_service_with_connect_info::<SocketAddr>();
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            service,
            store,
            faults,
            notifier,
            clock,
            address,
            router,
        }
    }

    /// A transport signed in as `member` of the team.
    pub fn signed_in(&self, member: &str) -> Transport {
        self.bearer(&format!("team-{member}"))
    }

    /// The typed client, signed in as `member` of the team.
    pub fn client(&self, member: &str) -> cairn_api::client::Client {
        cairn_api::client::Client::new(self.signed_in(member))
    }

    /// A transport presenting `token`.
    pub fn bearer(&self, token: &str) -> Transport {
        Transport::new(self.address, Some(token.to_owned()))
    }

    /// Publishes the vendor evaluation's route as `ann` and runs its scenario's first
    /// `steps` steps, each at its own time; answers `ann`'s transport.
    pub async fn vendor_after(&self, steps: usize) -> Transport {
        let ann = self.signed_in("ann");
        let (people_call, people) = crate::people::people_write();
        self.service.patch(&people_call, &people).await.unwrap();
        let seed = publish_fixture_route("vendor-evaluation");
        let reply = post(
            &ann,
            "/api/routes/vendor-evaluation/patches",
            &request(&seed),
        )
        .await;
        ok::<serde_json::Value>(&reply);
        for step in scenario("vendor-evaluation")
            .steps
            .as_slice()
            .iter()
            .take(steps)
        {
            self.clock.set_to(step.at);
            let body = serde_json::json!({ "patch": step.patch, "note": step.note });
            ok::<serde_json::Value>(
                &post(&ann, "/api/journeys/j_vendor_eval/patches", &body).await,
            );
        }
        ann
    }

    /// The journey's revision as the store holds it.
    pub async fn revision(&self, journey: &str) -> u32 {
        let journey = self
            .service
            .journey(&journey.parse().unwrap())
            .await
            .unwrap();
        journey.unwrap().revision.get()
    }

    /// A transport with no credentials.
    pub fn anonymous(&self) -> Transport {
        Transport::new(self.address, None)
    }
}

/// A clock at the vendor evaluation's first step.
fn start_clock() -> TestClock {
    let clock = TestClock::default();
    clock.set("2026-10-01T14:00:00Z");
    clock
}

/// Who stands in front of the server and signs requests in.
enum Front {
    /// The test sign-in provider for the team, and the built-in OAuth server.
    Team,
    /// The IAP provider with its relay, behind a fake IAP.
    Iap(Box<(FakeIap, GoogleEndpoints)>),
}

impl Front {
    fn providers(
        &self,
        accounts: &Accounts,
        base: &Url,
        clock: Clock,
    ) -> (Vec<Arc<dyn AuthProvider>>, Vec<AuthMethod>) {
        match self {
            Front::Team => {
                let oauth = OAuthConfig {
                    name: "cairn".parse().unwrap(),
                    sign_in_with: None,
                };
                let providers: Vec<Arc<dyn AuthProvider>> = vec![
                    Arc::new(TeamProvider {
                        name: "team".parse().unwrap(),
                    }),
                    Arc::new(OAuthServer::new(oauth, accounts.clone(), base)),
                ];
                let methods = vec![
                    method("team", AuthKind::Oidc),
                    method("cairn", AuthKind::BuiltinOauth),
                ];
                (providers, methods)
            }
            Front::Iap(iap) => {
                let endpoints = &iap.1;
                let config = GcpIapConfig {
                    name: "iap".parse().unwrap(),
                    audience: AUDIENCE.to_owned(),
                    auto_link: false,
                    relay: Some(RelayClient {
                        client_id: GOOGLE_CLIENT.to_owned(),
                        client_secret: GOOGLE_SECRET.to_owned(),
                    }),
                };
                let iap = GcpIapProvider::with_endpoints(config, clock, base, endpoints.clone());
                let providers: Vec<Arc<dyn AuthProvider>> = vec![Arc::new(iap.unwrap())];
                (providers, vec![method("iap", AuthKind::GcpIap)])
            }
        }
    }

    /// The server as reached from outside.
    fn in_front_of(&self, router: axum::Router) -> axum::Router {
        match self {
            Front::Team => router,
            Front::Iap(iap) => iap.0.in_front_of(router),
        }
    }
}

fn method(name: &str, kind: AuthKind) -> AuthMethod {
    AuthMethod {
        name: name.parse().unwrap(),
        kind,
    }
}

/// GETs `target` and parses a 200's body.
pub async fn get<T: DeserializeOwned>(transport: &Transport, target: &str) -> T {
    let reply = transport.send(Method::GET, target, None).await.unwrap();
    ok(&reply)
}

/// POSTs `body` to `target` and answers the reply, whatever its status.
pub async fn post(transport: &Transport, target: &str, body: &impl serde::Serialize) -> Reply {
    let body = serde_json::to_value(body).unwrap();
    transport
        .send(Method::POST, target, Some(&body))
        .await
        .unwrap()
}

/// A 2xx reply's body, or a panic showing what came back.
pub fn ok<T: DeserializeOwned>(reply: &Reply) -> T {
    assert!(
        reply.status.is_success(),
        "{}: {}",
        reply.status,
        String::from_utf8_lossy(&reply.body)
    );
    reply.json().unwrap()
}

/// The repository's fixtures directory.
pub fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// A fixture's journey scenario.
pub fn scenario(name: &str) -> Scenario {
    let path = fixtures_root().join(name).join("journey.yaml");
    from_yaml(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

/// The patch that creates a fixture's route and publishes its file as version 1, as every
/// scenario expects (fixtures/README.md).
pub fn publish_fixture_route(name: &str) -> Patch {
    let route = fixtures_root().join(name).join("route.yaml");
    let path = if route.exists() {
        route
    } else {
        fixtures_root().join(name).join("segment.yaml")
    };
    let file: RouteFile = from_yaml(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let graph = from_file(&file, &mut SequentialKeys::default())
        .unwrap()
        .into_document();
    let mut mutations = vec![
        Mutation::CreateRoute {
            name: file.name.clone(),
            description: file.description.clone(),
            kind: file.kind,
        },
        Mutation::OpenDraft {
            source: DraftSource::Import,
        },
    ];
    let roles = graph.roles.values().cloned();
    mutations.extend(roles.map(|role| Mutation::AddRole { role }));
    let kinds = graph.participation_kinds.values().cloned();
    mutations.extend(kinds.map(|kind| Mutation::AddParticipationKind { kind }));
    if let Some(role) = &graph.default_owner {
        mutations.push(Mutation::SetDefaultOwner {
            role: Some(role.clone()),
        });
    }
    let nodes = graph.nodes.values().cloned();
    mutations.extend(nodes.map(|node| Mutation::AddNode { node }));
    mutations.push(Mutation::PublishDraft {});
    Patch {
        id: format!("p_seed_{}", name.replace('-', "_"))
            .parse()
            .unwrap(),
        target: PatchTarget::Route(file.route.clone()),
        base_revision: Revision::NONE,
        deployment_revision: None,
        mutations: Mutations::new(mutations).unwrap(),
    }
}

/// A patch from YAML: `id`, `target` (as YAML), the base revision, and the mutations.
pub fn patch(id: &str, target: &str, base: u32, mutations: &str) -> Patch {
    let yaml =
        format!("id: {id}\ntarget: {target}\nbase_revision: {base}\nmutations:\n{mutations}");
    from_yaml(&yaml).unwrap_or_else(|error| panic!("{error}\n{yaml}"))
}

/// The body of `POST /api/{domain}/patches` for `patch`, with no note.
pub fn request(patch: &Patch) -> serde_json::Value {
    serde_json::json!({ "patch": patch })
}

/// A proposal for each kind of destination, as `(create endpoint, its path, proposal id,
/// draft)`, against the vendor evaluation after kickoff: the journey's adds a charter that
/// kickoff, already done, requires, so its preview has consequences (D7); the route's
/// creates a route at revision 0; the deployment's creates an identity.
pub async fn proposals_of_every_domain(
    transport: &Transport,
) -> Vec<(
    &'static cairn_api::endpoints::Endpoint,
    String,
    &'static str,
    serde_json::Value,
)> {
    use cairn_api::endpoints as at;
    let journey: cairn_schema::Journey = get(transport, "/api/journeys/j_vendor_eval").await;
    let deployment: cairn_schema::Deployment = get(transport, "/api/deployment").await;
    let charter = serde_json::json!({
        "title": "Charter first",
        "destination_revision": journey.revision,
        "mutations": [
            {"op": "add_node", "node": {"key": "n_charter", "id": "charter", "kind": "action", "title": "Charter"}},
            {"op": "add_edge", "edge": {"node": "n_kickoff", "requires": "n_charter"}},
        ],
    });
    let route = serde_json::json!({
        "title": "A new route",
        "destination_revision": 0,
        "mutations": [{"op": "create_route", "name": "New route"}],
    });
    let identity = serde_json::json!({
        "title": "A new person",
        "destination_revision": deployment.revision,
        "mutations": [{"op": "create_identity", "identity": {"key": "who_new", "name": "New person"}}],
    });
    vec![
        (
            &at::PROPOSE_JOURNEY,
            "/api/journeys/j_vendor_eval/proposals".to_owned(),
            "pr_charter",
            charter,
        ),
        (
            &at::PROPOSE_ROUTE,
            "/api/routes/new-route/proposals".to_owned(),
            "pr_route",
            route,
        ),
        (
            &at::PROPOSE_DEPLOYMENT,
            "/api/deployment/proposals".to_owned(),
            "pr_identity",
            identity,
        ),
    ]
}
