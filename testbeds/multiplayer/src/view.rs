//! A client's view: what it fetched of the journey and the deployment, kept current by its
//! SSE subscription through H6's tracking (`cairn_api::client::Tracker`). A tick newer than
//! what the view holds makes it refetch; a stream that ends or fails is opened again, and
//! its first ticks (the current revisions) bring the view up to date with anything it
//! missed while away.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::time::Duration;

use cairn_api::client::Tracker;
use cairn_api::query::WatchName;
use cairn_api::wire::Tick;
use cairn_schema::{Domain, Revision, RevisionOf};
use cairn_store::limits::SSE_COALESCING_INTERVAL;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::client::{ATTEMPT_TIMEOUT, Client, GaveUp};
use crate::world::Setup;

/// How long after writes stop every view may take to hold the store's revisions: the
/// coalescing interval (H6: the last commit's tick waits at most that long), plus one
/// attempt's budget for the tick to cross the network and the refetch to come back.
pub const CATCH_UP_DEADLINE: Duration = SSE_COALESCING_INTERVAL.saturating_add(ATTEMPT_TIMEOUT);
/// The pause before a view opens its stream again.
const RECONNECT_BACKOFF: Duration = Duration::from_millis(5);

/// The revisions a view holds, by domain.
pub type Held = BTreeMap<RevisionOf, Revision>;

/// A view, following its subscription on a task of its own.
pub struct View {
    client: u32,
    held: watch::Receiver<Held>,
    task: JoinHandle<()>,
}

impl View {
    /// Opens client `client`'s view of the journey and the deployment.
    pub fn open(address: SocketAddr, client: u32, setup: &Setup) -> Self {
        let (holds, held) = watch::channel(Held::new());
        let follower = Follower {
            client: Client::new(address, client),
            setup: setup.clone(),
            tracker: Tracker::new(),
            holds,
        };
        Self {
            client,
            held,
            task: tokio::spawn(follower.follow()),
        }
    }

    /// Stops following.
    pub fn close(self) {
        self.task.abort();
    }
}

/// How each view fared after writes stopped: the virtual time it took to hold `target`, or
/// what it held at the deadline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaughtUp {
    pub client: u32,
    pub outcome: Result<Duration, Held>,
}

/// Waits until every view holds at least `target`, or [`CATCH_UP_DEADLINE`] has passed
/// since this was called.
pub async fn catch_up(views: &mut [View], target: &Held) -> Vec<CaughtUp> {
    let started = Instant::now();
    let deadline = started + CATCH_UP_DEADLINE;
    let mut caught = Vec::new();
    for view in views {
        let holds = |held: &Held| {
            target
                .iter()
                .all(|(of, revision)| held.get(of).is_some_and(|held| held >= revision))
        };
        let waited = tokio::time::timeout_at(deadline, view.held.wait_for(holds))
            .await
            .is_ok_and(|held| held.is_ok());
        let outcome = if waited {
            Ok(started.elapsed())
        } else {
            Err(view.held.borrow().clone())
        };
        caught.push(CaughtUp {
            client: view.client,
            outcome,
        });
    }
    caught
}

/// The task behind a view.
struct Follower {
    client: Client,
    setup: Setup,
    tracker: Tracker,
    holds: watch::Sender<Held>,
}

impl Follower {
    fn journey(&self) -> RevisionOf {
        RevisionOf::Domain(Domain::Journey(self.setup.journey.clone()))
    }

    /// Fetches both domains, then follows the stream, opening it again whenever it ends.
    async fn follow(mut self) {
        for of in [self.journey(), RevisionOf::Domain(Domain::Deployment)] {
            self.refetch_until_held(&of).await;
        }
        let watching: Vec<WatchName> = [
            format!("journey:{}", self.setup.journey),
            "deployment".to_owned(),
        ]
        .iter()
        .filter_map(|name| name.parse().ok())
        .collect();
        loop {
            let opened =
                tokio::time::timeout(ATTEMPT_TIMEOUT, self.client.subscribe(&watching)).await;
            if let Ok(Ok(subscription)) = opened {
                self.read(subscription).await;
            }
            tokio::time::sleep(RECONNECT_BACKOFF).await;
        }
    }

    /// Reads one stream until it ends or fails.
    async fn read(&mut self, mut subscription: cairn_api::client::Subscription) {
        let mut last: BTreeMap<RevisionOf, Revision> = BTreeMap::new();
        while let Ok(Some(tick)) = subscription.next_tick().await {
            self.heard(&tick, last.get(&tick.of).copied());
            last.insert(tick.of.clone(), tick.revision);
            if self.tracker.ticked(&tick) {
                self.refetch_until_held(&tick.of).await;
            }
        }
    }

    /// The coverage oracles a tick can satisfy.
    fn heard(&self, tick: &Tick, previous: Option<Revision>) {
        // Within one stream, a tick that skips revisions stands for several commits.
        let coalesced = previous.is_some_and(|previous| tick.revision.get() > previous.get() + 1);
        patina_dst::sometimes!(coalesced, "view-coalesced-tick");
        // Only a merge moves the deployment once setup is done.
        let merged = tick.of == RevisionOf::Domain(Domain::Deployment)
            && tick.revision > self.setup.deployment_revision;
        patina_dst::sometimes!(merged, "view-deployment-tick-after-merge");
    }

    /// Refetches `of` until a fetch lands, and records what it holds.
    async fn refetch_until_held(&mut self, of: &RevisionOf) {
        loop {
            match self.fetch(of).await {
                Ok(revision) => {
                    self.tracker.fetched(of, revision);
                    let held = self.tracker.held(of).unwrap_or(revision);
                    self.holds.send_modify(|holds| {
                        holds.insert(of.clone(), held);
                    });
                    return;
                }
                Err(_) => tokio::time::sleep(RECONNECT_BACKOFF).await,
            }
        }
    }

    async fn fetch(&self, of: &RevisionOf) -> Result<Revision, GaveUp> {
        match of {
            RevisionOf::Domain(Domain::Deployment) => Ok(self.client.deployment().await?.revision),
            _ => Ok(self.client.journey(&self.setup).await?.revision),
        }
    }
}
