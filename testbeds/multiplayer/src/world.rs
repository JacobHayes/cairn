//! The world the clients share: a composition root like the binary's (4.7) - the Turso
//! store, the in-process notifier, the dev sign-in provider, and the API's router - served
//! on the simulated loopback network, and the journey every client patches.

use std::collections::BTreeSet;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use cairn_auth::{Accounts, Auth, AuthProvider, Clock, DevConfig, DevProvider, Listener};
use cairn_schema::{
    EntityKey, JourneyId, NodeKey, PatchReceipt, RankConstants, Revision, TouchedSet,
};
use cairn_service::{AuthKind, AuthMethod, Capabilities, DeploymentSettings, Parts, Service};
use cairn_store::InProcessNotifier;
use cairn_store_turso::TursoStore;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::Stop;
use crate::client::Client;
use crate::faults::Sites;
use crate::patches;

/// The server's address: an IP literal, since the shim resolves no names without a host
/// table.
pub const SERVER: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 7878));
/// The database file, in the run's own virtual filesystem.
const DATABASE: &str = "/multiplayer.db";
/// How long a graceful server shutdown may take once every client and view is done.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);
/// Nodes in the shared journey: few, so edits to one node's title collide often.
pub const NODE_COUNT: u32 = 3;
/// Entity pairs set up for merges: the first client merges one pair mid-run.
pub const MERGE_COUNT: u32 = 1;

/// The server, serving until stopped.
pub struct Server {
    sites: Arc<Sites>,
    service: Service<TursoStore>,
    stop: oneshot::Sender<()>,
    task: JoinHandle<std::io::Result<()>>,
}

impl Server {
    /// Opens the store, assembles the service and auth, and serves the API at [`SERVER`].
    ///
    /// # Errors
    ///
    /// An abort when the store or the listener cannot open.
    pub async fn start() -> Result<Self, Stop> {
        let (sites, faults) = Sites::hooked();
        let store = TursoStore::open_with_faults(Path::new(DATABASE), faults)
            .await
            .map_err(|error| Stop::Abort {
                label: "store",
                detail: error.to_string(),
            })?;
        let store = Arc::new(store);
        let listener = tokio::net::TcpListener::bind(SERVER)
            .await
            .map_err(|error| Stop::Abort {
                label: "bind",
                detail: error.to_string(),
            })?;
        let auth = auth(&store)?;
        let settings = DeploymentSettings::new(
            "UTC".parse().map_err(|error| Stop::Abort {
                label: "timezone",
                detail: format!("{error:?}"),
            })?,
            jiff::tz::TimeZone::UTC,
            RankConstants::default(),
        );
        let methods = vec![AuthMethod {
            name: slug("dev"),
            kind: AuthKind::Dev,
        }];
        let service = Service::new(Parts {
            store: Arc::clone(&store),
            notifier: Arc::new(InProcessNotifier::new()),
            settings,
            capabilities: Capabilities::server(methods, false),
        });
        let app = cairn_api::router(service.clone(), &auth)
            .into_make_service_with_connect_info::<SocketAddr>();
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    // A dropped sender means the run is over either way.
                    let _ = stopped.await;
                })
                .await
        });
        Ok(Self {
            sites,
            service,
            stop,
            task,
        })
    }

    /// Arms the store's fault sites: setup is done.
    pub fn arm(&self) {
        self.sites.arm();
    }

    /// The service behind it, which the judge reads the store's truth through.
    pub fn service(&self) -> &Service<TursoStore> {
        &self.service
    }

    /// Stops serving and waits for in-flight connections to finish.
    ///
    /// # Errors
    ///
    /// A liveness miss when the server fails or does not stop in time.
    pub async fn stop(self) -> Result<(), Stop> {
        let _ = self.stop.send(());
        match tokio::time::timeout(SHUTDOWN_TIMEOUT, self.task).await {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(error))) => Err(Stop::Liveness(format!("server failed: {error}"))),
            Ok(Err(error)) => Err(Stop::Liveness(format!("server task failed: {error}"))),
            Err(_) => Err(Stop::Liveness("server did not stop".to_owned())),
        }
    }
}

/// Auth as the binary's root assembles it for local development: the dev sign-in provider
/// on a loopback listener, with no token, so every request is the dev user (H1).
fn auth(store: &Arc<TursoStore>) -> Result<Auth<TursoStore>, Stop> {
    let invalid = |label: &'static str| {
        move |error: &dyn std::fmt::Debug| Stop::Abort {
            label,
            detail: format!("{error:?}"),
        }
    };
    let base = url::Url::parse(&format!("http://{SERVER}"))
        .map_err(|error| invalid("base-url")(&error))?;
    let dev = DevConfig {
        name: slug("dev"),
        user: "Tester"
            .parse()
            .map_err(|error| invalid("dev-user")(&error))?,
        verified_emails: BTreeSet::new(),
        token: None,
        auto_link: false,
        allow_off_loopback: false,
    };
    let provider = DevProvider::new(dev, Listener::Tcp(SERVER))
        .map_err(|error| invalid("dev-provider")(&error))?;
    let providers: Vec<Arc<dyn AuthProvider>> = vec![Arc::new(provider)];
    let accounts = Accounts::new(Arc::clone(store), Clock::system());
    Ok(Auth::new(accounts, Listener::Tcp(SERVER), &base, providers))
}

/// A slug the testbed names, known valid.
fn slug(text: &str) -> cairn_schema::Slug {
    match text.parse() {
        Ok(slug) => slug,
        Err(error) => unreachable!("{text:?} is a slug: {error:?}"),
    }
}

/// What every client starts from: the shared journey and its nodes, the entities set up to
/// be merged, and the revisions setup left.
#[derive(Clone, Debug)]
pub struct Setup {
    pub journey: JourneyId,
    pub nodes: Vec<NodeKey>,
    /// Each pair is (survivor, merged).
    pub merges: Vec<(EntityKey, EntityKey)>,
    pub deployment_revision: Revision,
    /// What setup's own patches were acknowledged with, and what each touches.
    pub receipts: Vec<(PatchReceipt, TouchedSet)>,
}

/// Creates the shared journey and the entities to merge, through the API.
///
/// # Errors
///
/// A liveness miss when setup cannot land.
pub async fn set_up(client: &Client) -> Result<Setup, Stop> {
    let created = client.land(patches::create_journey(NODE_COUNT)).await?;
    let entities = client.land(patches::create_entities(MERGE_COUNT)).await?;
    Ok(Setup {
        journey: patches::journey(),
        nodes: (0..NODE_COUNT).map(patches::node).collect(),
        merges: (0..MERGE_COUNT).map(patches::merge_pair).collect(),
        deployment_revision: entities.0.revision,
        receipts: vec![created, entities],
    })
}
