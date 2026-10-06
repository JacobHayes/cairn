//! A virtual client: the real Rust client (`cairn_api::client`) over HTTP, with a transport
//! retry under H5's safe retry, drafting patches against the journey or deployment it last
//! fetched.
//!
//! Two retries stack, as a real client's would. Inside, a request whose answer did not
//! arrive (a reset, a refused connect, a timeout, or the `client-loses-response` fault site)
//! is sent again unchanged: the same patch id with the same content, which the server
//! answers from its receipt if the first copy landed (H5). Outside, `retry::submit`
//! resubmits a stale patch at the revisions its rejection reports when what intervened
//! cannot overlap it, and surfaces it when it can.

use std::fmt;
use std::future::Future;
use std::hash::{BuildHasher, RandomState};
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::sync::watch;

use axum::http::StatusCode;
use cairn_api::client::{self as api, ClientError, Landed, Refused, Transport, retry};
use cairn_api::wire::PatchAnswer;
use cairn_schema::{
    Actor, Deployment, Domain, Journey, Patch, PatchId, PatchReceipt, Rejection, Revision,
    Timestamp, TouchedSet, UserId,
};
use cairn_service::{Call, DomainPatch, Service, WriteError};
use cairn_store_turso::TursoStore;

use crate::Stop;
use crate::patches::Draft;
use crate::world::{NODE_COUNT, Setup};

/// One attempt's budget: connect, send, and read the whole answer. Many times a round trip
/// under the sweep's millisecond-scale delays, so only stacked delays or a lost answer time
/// an attempt out.
pub const ATTEMPT_TIMEOUT: Duration = Duration::from_millis(200);
/// The pause before an attempt is sent again.
const RETRY_BACKOFF: Duration = Duration::from_millis(5);
/// Attempts per request before the client gives up (a liveness miss).
const ATTEMPTS_MAX: u32 = 64;
/// The longest pause between a client's patches: short, so drafts collide.
const THINK_MS_MAX: u64 = 20;

/// A request that failed every attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GaveUp {
    pub what: String,
    pub last_error: String,
}

impl fmt::Display for GaveUp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "gave up on {}: {}", self.what, self.last_error)
    }
}

/// Why a patch got no answer the client can act on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// Every attempt failed in transit or timed out.
    GaveUp(GaveUp),
    /// The server answered an error no retry fixes (a 500, a 400): a finding.
    Server(String),
}

/// A patch the server acknowledged, and what its author's next snapshot showed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ack {
    pub receipt: PatchReceipt,
    /// The revision it was drafted at, before any automatic resubmission.
    pub drafted: Revision,
    /// What it touches (H5).
    pub touched: TouchedSet,
    /// The next snapshot of its domain held it: at or past its revision, with its effect.
    pub visible: bool,
}

/// What one client saw.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub acks: Vec<Ack>,
    /// Patches answered stale where retrying was not safe: surfaced, so never applied.
    pub surfaced: Vec<PatchId>,
    /// Of those, the ones surfaced only because the resubmission bound ran out: what
    /// intervened did not overlap them.
    pub surfaced_exhausted: u64,
    /// Any other rejection or server error, which none of the testbed's patches should get.
    pub unexpected: Vec<(PatchId, String)>,
    /// Requests sent again because an answer did not arrive.
    pub transport_retries: u64,
    /// Stale patches resubmitted on their own (H5).
    pub resubmissions: u64,
    /// Answers from a receipt: a resent patch had already landed.
    pub receipts: u64,
}

impl Report {
    /// Adds what `other` counted.
    pub fn absorb(&mut self, other: Report) {
        self.acks.extend(other.acks);
        self.surfaced.extend(other.surfaced);
        self.surfaced_exhausted += other.surfaced_exhausted;
        self.unexpected.extend(other.unexpected);
        self.transport_retries += other.transport_retries;
        self.resubmissions += other.resubmissions;
        self.receipts += other.receipts;
    }
}

/// Counters a request updates as it goes.
#[derive(Default)]
struct Counts {
    transport_retries: AtomicU64,
    receipts: AtomicU64,
}

/// How a client reaches the service.
#[derive(Clone, Debug)]
enum Channel {
    /// Over HTTP, through the real Rust client.
    Http(api::Client),
    /// In process, as an agent host (MCP, the assistant) calls the service. Each attempt
    /// runs on a task of its own, and an attempt the agent stops waiting for keeps running,
    /// as a server-side call does once its caller has gone: so a resubmission can meet its
    /// original in flight, which an HTTP client cannot arrange (the server drops a request
    /// whose connection closes).
    InProcess {
        service: Service<TursoStore>,
        /// Attempts still running, which the agent waits out before it reports.
        in_flight: watch::Sender<usize>,
    },
}

/// A client of the server, numbered for its patch ids.
#[derive(Clone, Debug)]
pub struct Client {
    channel: Channel,
    index: u32,
}

impl Client {
    /// Client `index` of the server at `address`. The dev sign-in provider needs no token.
    pub fn new(address: SocketAddr, index: u32) -> Self {
        Self {
            channel: Channel::Http(api::Client::new(Transport::new(address, None))),
            index,
        }
    }

    /// Client `index` calling `service` in process, as an agent host would.
    pub fn in_process(service: Service<TursoStore>, index: u32) -> Self {
        Self {
            channel: Channel::InProcess {
                service,
                in_flight: watch::Sender::new(0),
            },
            index,
        }
    }

    /// Runs `attempt` until it answers within [`ATTEMPT_TIMEOUT`], at most [`ATTEMPTS_MAX`]
    /// times; an attempt answers `Err` to be tried again.
    async fn with_retries<T, F, Fut>(
        &self,
        what: &str,
        counts: &Counts,
        mut attempt: F,
    ) -> Result<T, GaveUp>
    where
        F: FnMut(u32) -> Fut,
        Fut: Future<Output = Result<T, String>>,
    {
        let mut last_error = String::new();
        for made in 1..=ATTEMPTS_MAX {
            let outcome = tokio::time::timeout(ATTEMPT_TIMEOUT, attempt(made))
                .await
                .unwrap_or_else(|_| Err("timed out".to_owned()));
            match outcome {
                Ok(answer) => return Ok(answer),
                Err(error) => last_error = error,
            }
            counts.transport_retries.fetch_add(1, Ordering::Relaxed);
            tokio::time::sleep(RETRY_BACKOFF).await;
        }
        Err(GaveUp {
            what: what.to_owned(),
            last_error,
        })
    }

    /// The journey, read with retries.
    pub async fn journey(&self, setup: &Setup) -> Result<Journey, GaveUp> {
        match &self.channel {
            Channel::Http(api) => {
                self.read("the journey", || api.journey(&setup.journey))
                    .await
            }
            Channel::InProcess { service, .. } => match service.journey(&setup.journey).await {
                Ok(Some(journey)) => Ok(journey),
                Ok(None) => Err(gave_up("the journey", "it does not exist")),
                Err(error) => Err(gave_up("the journey", &error.to_string())),
            },
        }
    }

    /// H6: opens a subscription to the ticks of what `watching` names, once.
    ///
    /// # Errors
    ///
    /// A refusal or a transport failure.
    pub async fn subscribe(
        &self,
        watching: &[cairn_api::query::WatchName],
    ) -> Result<api::Subscription, ClientError> {
        match &self.channel {
            Channel::Http(api) => api.subscribe(watching).await,
            Channel::InProcess { .. } => Err(ClientError::Transport(api::TransportError(
                "an in-process client has no stream".to_owned(),
            ))),
        }
    }

    /// The deployment, read with retries.
    pub async fn deployment(&self) -> Result<Deployment, GaveUp> {
        match &self.channel {
            Channel::Http(api) => self.read("the deployment", || api.deployment()).await,
            Channel::InProcess { service, .. } => service
                .deployment()
                .await
                .map_err(|error| gave_up("the deployment", &error.to_string())),
        }
    }

    async fn read<T, F, Fut>(&self, what: &str, read: F) -> Result<T, GaveUp>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, ClientError>>,
    {
        self.with_retries(what, &Counts::default(), |_| async {
            read().await.map_err(|error| error.to_string())
        })
        .await
    }

    /// Submits `patch` as it is until an answer arrives: an attempt that failed in transit
    /// is sent again unchanged, so a copy that landed is answered from its receipt.
    async fn submit(&self, patch: Patch, counts: &Counts) -> Result<PatchAnswer, Refused<Failure>> {
        let what = format!("patch {}", patch.id);
        let answered = self
            .with_retries(&what, counts, |attempt| {
                let sent = self.send(patch.clone());
                async move {
                    match sent.await {
                        Ok(answer) => {
                            // A first answer lost on its way back: the server applied it,
                            // the client never heard.
                            if attempt == 1 && patina_dst::buggify!("client-loses-response") {
                                return Err("answer lost (fault site)".to_owned());
                            }
                            Ok(Ok(answer))
                        }
                        Err(Refused::Rejected(rejection)) => Ok(Err(Refused::Rejected(rejection))),
                        Err(Refused::Failed(error)) if retriable(&error) => Err(error.to_string()),
                        Err(Refused::Failed(error)) => {
                            Ok(Err(Refused::Failed(Failure::Server(error.to_string()))))
                        }
                    }
                }
            })
            .await
            .map_err(|gave_up| Refused::Failed(Failure::GaveUp(gave_up)))?;
        match answered {
            Ok(answer) => {
                let from_receipt = matches!(answer, PatchAnswer::AlreadyApplied { .. });
                patina_dst::sometimes!(from_receipt, "client-resubmission-answered-from-receipt");
                if from_receipt {
                    counts.receipts.fetch_add(1, Ordering::Relaxed);
                }
                Ok(answer)
            }
            Err(refused) => Err(refused),
        }
    }

    /// One attempt to submit `patch`. An in-process attempt runs on its own task, so the
    /// returned future may be dropped while the attempt goes on.
    fn send(
        &self,
        patch: Patch,
    ) -> Pin<Box<dyn Future<Output = Result<PatchAnswer, Refused<ClientError>>> + Send>> {
        match &self.channel {
            Channel::Http(api) => {
                let api = api.clone();
                Box::pin(async move { api.submit(patch, None).await })
            }
            Channel::InProcess { service, in_flight } => {
                let service = service.clone();
                in_flight.send_modify(|count| *count += 1);
                let in_flight = in_flight.clone();
                let call = Call {
                    actor: Actor {
                        user: agent_user(),
                        agent: None,
                    },
                    now: Timestamp::now(),
                };
                let attempt = tokio::spawn(async move {
                    let answer = match DomainPatch::new(patch, None) {
                        Err(_) => Err(Refused::Failed(ClientError::NotADomainPatch)),
                        Ok(submitted) => match service.patch(&call, &submitted).await {
                            Ok(written) => Ok(written.into()),
                            Err(WriteError::Rejected(rejection)) => {
                                Err(Refused::Rejected(rejection))
                            }
                            Err(WriteError::Failed(error)) => {
                                Err(Refused::Failed(server_error(&error.to_string())))
                            }
                        },
                    };
                    in_flight.send_modify(|count| *count -= 1);
                    answer
                });
                Box::pin(async move {
                    attempt.await.unwrap_or_else(|error| {
                        Err(Refused::Failed(server_error(&error.to_string())))
                    })
                })
            }
        }
    }

    /// H5: submits `patch`, resubmitting it while it is stale and safe to retry, each
    /// submission with the transport retry.
    async fn patch(&self, patch: Patch, counts: &Counts) -> Result<Landed, Refused<Failure>> {
        retry::submit(patch, |patch| self.submit(patch, counts)).await
    }

    /// Lands a setup patch, which nothing can conflict with.
    ///
    /// # Errors
    ///
    /// A liveness miss when it does not land.
    pub async fn land(&self, patch: Patch) -> Result<(PatchReceipt, TouchedSet), Stop> {
        let touched = patch.touched();
        match self.patch(patch, &Counts::default()).await {
            Ok(landed) => Ok((landed.answer.receipt().clone(), touched)),
            Err(Refused::Failed(Failure::GaveUp(gave_up))) => Err(gave_up.into()),
            Err(Refused::Failed(Failure::Server(error))) => {
                Err(Stop::Liveness(format!("setup failed: {error}")))
            }
            Err(Refused::Rejected(rejection)) => {
                Err(Stop::Liveness(format!("setup was rejected: {rejection:?}")))
            }
        }
    }

    /// Drafts and lands `actions` patches, one after another, each against the domain as
    /// the client last fetched it, pausing a seeded moment between them.
    ///
    /// # Errors
    ///
    /// When a request fails every attempt.
    pub async fn run(&self, setup: &Setup, actions: u32) -> Result<Report, GaveUp> {
        let mut random = Random::new(self.index);
        let mut report = Report::default();
        for sequence in 0..actions {
            let think = Duration::from_millis(random.below(THINK_MS_MAX + 1));
            tokio::time::sleep(think).await;
            let draft = self.draft(setup, sequence, actions, &mut random);
            let base = if draft.is_deployment() {
                self.deployment().await?.revision
            } else {
                self.journey(setup).await?.revision
            };
            let id = format!("p_c{}_{sequence}", self.index);
            let patch = draft.patch(&id, &setup.journey, base);
            let counts = Counts::default();
            let answer = self.patch(patch.clone(), &counts).await;
            report.transport_retries += counts.transport_retries.load(Ordering::Relaxed);
            report.receipts += counts.receipts.load(Ordering::Relaxed);
            match answer {
                Ok(landed) => {
                    patina_dst::sometimes!(
                        landed.resubmitted > 0,
                        "client-stale-patch-retried-and-landed"
                    );
                    report.resubmissions += u64::from(landed.resubmitted);
                    let receipt = landed.answer.receipt().clone();
                    let visible = self.next_snapshot_holds(setup, &draft, &receipt).await?;
                    report.acks.push(Ack {
                        receipt,
                        drafted: patch.base_revision,
                        touched: patch.touched(),
                        visible,
                    });
                }
                Err(Refused::Rejected(Rejection::Stale { intervening, .. })) => {
                    let overlapping = intervening.overlaps(&patch.touched());
                    patina_dst::sometimes!(overlapping, "client-overlapping-conflict-surfaced");
                    if !overlapping {
                        report.surfaced_exhausted += 1;
                    }
                    report.surfaced.push(patch.id);
                }
                Err(Refused::Rejected(rejection)) => {
                    report.unexpected.push((patch.id, format!("{rejection:?}")));
                }
                Err(Refused::Failed(Failure::Server(error))) => {
                    report.unexpected.push((patch.id, error));
                }
                Err(Refused::Failed(Failure::GaveUp(gave_up))) => return Err(gave_up),
            }
        }
        if let Channel::InProcess { in_flight, .. } = &self.channel {
            // Writes have not stopped while an abandoned attempt is still running.
            let _ = in_flight.subscribe().wait_for(|count| *count == 0).await;
        }
        Ok(report)
    }

    /// What client `index` drafts at `sequence`: the first client merges an entity pair
    /// halfway through; otherwise a note or a rename of one of the few nodes.
    fn draft(&self, setup: &Setup, sequence: u32, actions: u32, random: &mut Random) -> Draft {
        if self.index == 0
            && sequence == actions / 2
            && let Some((survivor, merged)) = setup.merges.first()
        {
            return Draft::Merge {
                survivor: survivor.clone(),
                merged: merged.clone(),
            };
        }
        if random.below(10) < 4 {
            return Draft::Annotate {
                key: format!("a_c{}_{sequence}", self.index),
            };
        }
        let node = usize::try_from(random.below(u64::from(NODE_COUNT))).unwrap_or(0);
        Draft::Rename {
            node: setup.nodes[node].clone(),
            title: format!("c{} a{sequence}", self.index),
        }
    }

    /// Whether the client's next fetch of the patch's domain holds what it acknowledged:
    /// at or past its revision, with its note or merge there (a rename may already be
    /// renamed again, so only its revision is checked here; the log check covers it).
    async fn next_snapshot_holds(
        &self,
        setup: &Setup,
        draft: &Draft,
        receipt: &PatchReceipt,
    ) -> Result<bool, GaveUp> {
        if receipt.domain == Domain::Deployment {
            let deployment = self.deployment().await?;
            let merged = match draft {
                Draft::Merge { survivor, merged } => {
                    deployment.aliases.get(merged) == Some(survivor)
                }
                _ => true,
            };
            return Ok(deployment.revision >= receipt.revision && merged);
        }
        let journey = self.journey(setup).await?;
        let noted = match draft {
            Draft::Annotate { key } => key
                .parse()
                .is_ok_and(|key| journey.graph.state.annotations.get(&key).is_some()),
            _ => true,
        };
        Ok(journey.revision >= receipt.revision && noted)
    }
}

fn gave_up(what: &str, error: &str) -> GaveUp {
    GaveUp {
        what: what.to_owned(),
        last_error: error.to_owned(),
    }
}

/// A service failure, as the HTTP client would report its 500.
fn server_error(message: &str) -> ClientError {
    ClientError::Unexpected {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        body: message.to_owned(),
    }
}

/// The user an in-process agent acts as.
fn agent_user() -> UserId {
    match "u_agent".parse() {
        Ok(user) => user,
        Err(error) => unreachable!("u_agent is a user id: {error:?}"),
    }
}

/// Whether a failed request is worth sending again: anything in transit, and the 503 every
/// server limit answers.
fn retriable(error: &ClientError) -> bool {
    match error {
        ClientError::Transport(_) => true,
        ClientError::Problem { status, .. } | ClientError::Unexpected { status, .. } => {
            *status == StatusCode::SERVICE_UNAVAILABLE
        }
        ClientError::NotADomainPatch => false,
    }
}

/// A small seeded generator: xorshift64*, seeded from std's hasher keys, which patina
/// draws from the run's seed, so each seed gives each client its own deterministic mix.
struct Random(u64);

impl Random {
    fn new(index: u32) -> Self {
        let seed = RandomState::new().hash_one(index) | 1;
        Self(seed)
    }

    /// A number in `0..bound`.
    fn below(&mut self, bound: u64) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) % bound.max(1)
    }
}
