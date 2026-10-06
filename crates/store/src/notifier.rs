//! The notifier (ARCHITECTURE, Storage > Concurrency and notification; H6): after every
//! commit the service announces each revision it moved, and every subscriber watching that
//! domain or proposal learns it. It is an in-process broadcast in both hosts: with one
//! process, every commit passes through it.
//!
//! A subscriber holds the latest revision per domain, not a queue, so ticks for one domain
//! coalesce however many commits land, and a slow subscriber never accumulates them. It is
//! handed what it holds at most once per `SSE_COALESCING_INTERVAL`, except its first take,
//! which is at once: on subscribe the current revisions are seeded (from the store, after
//! the subscription is registered), so a commit between a fetch and a subscription is never
//! missed; that first take is complete ([`Take::Current`]), so a domain deleted while the
//! subscriber was away shows as gone. A revision at or below one already handed over is
//! never handed over again.
//!
//! Runtime-free: the notifier reads no clock and starts no timer. The caller passes the
//! time to [`Subscription::take`], which answers when to come back, and awaits
//! [`Subscription::ready`] for something new, so the server's SSE loop and the browser's
//! event loop drive it alike.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use cairn_schema::{Domain, Revision, RevisionOf};

use crate::limits::{SSE_COALESCING_INTERVAL, SSE_SUBSCRIBER_COUNT_MAX};
use crate::query::Revisions;

/// What a subscriber watches.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Watch {
    /// One domain or proposal: a journey, route, proposal, or the deployment view.
    One(RevisionOf),
    /// Every journey: an index view, told which ones changed.
    Journeys,
    /// Every route.
    Routes,
    /// Every proposal.
    Proposals,
}

impl Watch {
    /// Whether a tick about `of` concerns this watch.
    #[must_use]
    pub fn covers(&self, of: &RevisionOf) -> bool {
        match self {
            Watch::One(watched) => watched == of,
            Watch::Journeys => matches!(of, RevisionOf::Domain(Domain::Journey(_))),
            Watch::Routes => matches!(of, RevisionOf::Domain(Domain::Route(_))),
            Watch::Proposals => matches!(of, RevisionOf::Proposal(_)),
        }
    }
}

/// One announcement: `of` is now at `revision` (H6). A subscriber refetches only when it is
/// newer than what it holds.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick {
    /// The domain or proposal.
    pub of: RevisionOf,
    /// Its revision. A domain that does not exist, or no longer does, is at revision 0.
    pub revision: Revision,
}

/// The process already has `SSE_SUBSCRIBER_COUNT_MAX` subscribers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubscriberLimit;

impl fmt::Display for SubscriberLimit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the process has {SSE_SUBSCRIBER_COUNT_MAX} subscribers (sse_subscriber_count_max)"
        )
    }
}

impl std::error::Error for SubscriberLimit {}

/// H6: announces new revisions to the subscribers of this process. A trait object at the
/// composition root (PRACTICES, No dynamic dispatch: the seams are the point).
pub trait Notifier: Send + Sync {
    /// Announces that `of` is now at `revision`: called after every commit, once for each
    /// revision the commit moved.
    fn publish(&self, of: &RevisionOf, revision: Revision);

    /// Registers a subscriber watching `watching`. It holds nothing until it is seeded with
    /// the current revisions ([`Subscription::seed`]) or a revision it watches is published.
    ///
    /// # Errors
    ///
    /// [`SubscriberLimit`] when the process already has `SSE_SUBSCRIBER_COUNT_MAX` live
    /// subscribers.
    fn subscribe(&self, watching: BTreeSet<Watch>) -> Result<Subscription, SubscriberLimit>;
}

/// The in-process notifier both hosts use: one broadcast to every live subscription.
#[derive(Debug, Default)]
pub struct InProcessNotifier {
    subscribers: Mutex<Vec<Weak<Mailbox>>>,
}

impl InProcessNotifier {
    /// A notifier with no subscribers.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many subscriptions are live (not yet dropped).
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        let mut subscribers = lock(&self.subscribers);
        subscribers.retain(|mailbox| mailbox.strong_count() > 0);
        subscribers.len()
    }
}

impl Notifier for InProcessNotifier {
    fn publish(&self, of: &RevisionOf, revision: Revision) {
        // The live mailboxes are collected under the lock and offered the tick after it, so
        // a publish never holds the notifier and a mailbox at once.
        let live: Vec<Arc<Mailbox>> = {
            let mut subscribers = lock(&self.subscribers);
            subscribers.retain(|mailbox| mailbox.strong_count() > 0);
            subscribers.iter().filter_map(Weak::upgrade).collect()
        };
        for mailbox in live {
            mailbox.offer(of, revision);
        }
    }

    fn subscribe(&self, watching: BTreeSet<Watch>) -> Result<Subscription, SubscriberLimit> {
        let mut subscribers = lock(&self.subscribers);
        subscribers.retain(|mailbox| mailbox.strong_count() > 0);
        let max = SSE_SUBSCRIBER_COUNT_MAX as usize;
        if subscribers.len() >= max {
            return Err(SubscriberLimit);
        }
        let mailbox = Arc::new(Mailbox {
            watching,
            state: Mutex::default(),
        });
        subscribers.push(Arc::downgrade(&mailbox));
        assert!(
            subscribers.len() <= max,
            "subscribers stay within the limit"
        );
        Ok(Subscription { mailbox })
    }
}

/// What a subscriber was handed by [`Subscription::take`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Take {
    /// The first take after [`Subscription::seed`]: the current revision of every watched
    /// domain or proposal that exists, and of each one watched by name (at 0 when it does
    /// not exist), with anything published since. It is complete, so a subscriber holding a
    /// domain this lists at 0, or one a kind watch covers that this does not list, knows it
    /// no longer exists (a hard delete, A19), even if the deletion's own tick was missed.
    Current(Vec<Tick>),
    /// The latest revision of each watched domain or proposal that moved, by domain.
    Ticks(Vec<Tick>),
    /// Something is held, but the last take was within `SSE_COALESCING_INTERVAL`: come back
    /// at `until`, on the same clock.
    Wait {
        /// When the next take hands it over.
        until: Duration,
    },
    /// Nothing new.
    Empty,
}

/// One subscriber's view of the notifier. Dropping it unsubscribes.
#[derive(Debug)]
pub struct Subscription {
    mailbox: Arc<Mailbox>,
}

impl Subscription {
    /// What it watches.
    #[must_use]
    pub fn watching(&self) -> &BTreeSet<Watch> {
        &self.mailbox.watching
    }

    /// H6: seeds the current revisions of everything it watches, as the store reported them
    /// after this subscription was registered: a commit published since is kept if it is
    /// newer, so none is missed either way. A domain or proposal watched by name that the
    /// store does not list is at revision 0.
    pub fn seed(&self, current: &Revisions) {
        let mut listed: Vec<Tick> = Vec::new();
        listed.push(Tick {
            of: RevisionOf::Domain(Domain::Deployment),
            revision: current.deployment,
        });
        listed.extend(current.journeys.iter().map(|(id, revision)| Tick {
            of: RevisionOf::Domain(Domain::Journey(id.clone())),
            revision: *revision,
        }));
        listed.extend(current.routes.iter().map(|(id, revision)| Tick {
            of: RevisionOf::Domain(Domain::Route(id.clone())),
            revision: *revision,
        }));
        listed.extend(current.proposals.iter().map(|(id, (_, revision))| Tick {
            of: RevisionOf::Proposal(id.clone()),
            revision: *revision,
        }));
        for tick in &listed {
            self.mailbox.offer(&tick.of, tick.revision);
        }
        for watch in &self.mailbox.watching {
            if let Watch::One(of) = watch
                && !listed.iter().any(|tick| tick.of == *of)
            {
                self.mailbox.offer(of, Revision::NONE);
            }
        }
        let mut state = lock(&self.mailbox.state);
        state.seeded = true;
        // The first take is ready now, even when nothing watched exists.
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }

    /// Hands over what it holds, at `now` on the caller's monotonic clock: at once on the
    /// first take (complete, as [`Take::Current`], once seeded), then at most once per
    /// `SSE_COALESCING_INTERVAL`.
    ///
    /// # Panics
    ///
    /// When `now` is earlier than the previous take's: the clock must not run backwards.
    #[must_use = "what is taken is handed over only once"]
    pub fn take(&self, now: Duration) -> Take {
        let mut state = lock(&self.mailbox.state);
        assert!(
            state.taken_at.is_none_or(|taken| now >= taken),
            "the subscriber's clock runs forwards"
        );
        let first = state.taken_at.is_none();
        if state.pending.is_empty() && !(first && state.seeded) {
            return Take::Empty;
        }
        if let Some(taken) = state.taken_at {
            let until = taken + SSE_COALESCING_INTERVAL;
            if now < until {
                return Take::Wait { until };
            }
        }
        let pending = std::mem::take(&mut state.pending);
        let mut ticks = Vec::with_capacity(pending.len());
        for (of, revision) in pending {
            let before = state.delivered.insert(of.clone(), revision);
            assert!(
                before.is_none_or(|before| before < revision),
                "a revision is handed over once, and only when newer"
            );
            ticks.push(Tick { of, revision });
        }
        state.taken_at = Some(now);
        assert!(state.pending.is_empty());
        if first && state.seeded {
            Take::Current(ticks)
        } else {
            assert!(!ticks.is_empty() && state.taken_at == Some(now));
            Take::Ticks(ticks)
        }
    }

    /// Resolves once the subscription holds something to take, including a seeded first take
    /// with nothing in it.
    pub fn ready(&self) -> Ready<'_> {
        Ready {
            mailbox: &self.mailbox,
        }
    }
}

/// The future [`Subscription::ready`] returns.
#[derive(Debug)]
#[must_use = "a future does nothing unless awaited"]
pub struct Ready<'a> {
    mailbox: &'a Mailbox,
}

impl Future for Ready<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        let mut state = lock(&self.mailbox.state);
        let first_after_seed = state.seeded && state.taken_at.is_none();
        if state.pending.is_empty() && !first_after_seed {
            state.waker = Some(context.waker().clone());
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

/// One subscriber's held revisions.
#[derive(Debug)]
struct Mailbox {
    watching: BTreeSet<Watch>,
    state: Mutex<MailboxState>,
}

#[derive(Debug, Default)]
struct MailboxState {
    /// The latest revision per domain or proposal not yet handed over.
    pending: BTreeMap<RevisionOf, Revision>,
    /// The latest revision handed over per domain or proposal.
    delivered: BTreeMap<RevisionOf, Revision>,
    /// When the subscriber last took its ticks.
    taken_at: Option<Duration>,
    /// Whether the current revisions were seeded, so the first take is complete.
    seeded: bool,
    /// Who to wake when something arrives.
    waker: Option<Waker>,
}

impl Mailbox {
    /// Holds `revision` for `of` if it is watched and newer than anything handed over or
    /// held: the latest wins, which is the coalescing.
    fn offer(&self, of: &RevisionOf, revision: Revision) {
        if !self.watching.iter().any(|watch| watch.covers(of)) {
            return;
        }
        let mut state = lock(&self.state);
        if state
            .delivered
            .get(of)
            .is_some_and(|delivered| *delivered >= revision)
        {
            return;
        }
        let held = state.pending.entry(of.clone()).or_insert(revision);
        *held = (*held).max(revision);
        assert!(state.pending.get(of).is_some_and(|held| *held >= revision));
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // Every critical section here leaves its state whole before anything that could panic.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
