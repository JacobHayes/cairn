//! Fault injection at named points of a commit (PRACTICES, Simulation with patina: fault
//! sites for what the runtime cannot inject from outside). Inert unless armed: a store built
//! with `Faults::default()` never fails or waits because of it. The conformance suite arms
//! a failure between a commit's state rows and its events to show nothing is left behind,
//! and a backend's own tests pause a commit midway to show what it does and does not block.

use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};

/// A point inside a commit, in the order a commit passes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CommitPoint {
    /// The domain's revision row is written (the commit's first write) and nothing else.
    AfterRevisionRow,
    /// The state rows are written and the events are not.
    BetweenStateAndEvents,
    /// Everything is written; the transaction has not committed.
    BeforeCommit,
}

/// What a paused commit waits on: called at every point, the future it returns is awaited
/// before the commit goes on.
pub type PauseHook =
    Arc<dyn Fn(CommitPoint) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// The faults armed on one store. Cloning shares them.
#[derive(Clone, Default)]
pub struct Faults {
    inner: Arc<Armed>,
}

#[derive(Default)]
struct Armed {
    failures: Mutex<BTreeSet<CommitPoint>>,
    pause: Mutex<Option<PauseHook>>,
}

impl Faults {
    /// The next commit to reach `point` fails there, once.
    pub fn fail_once(&self, point: CommitPoint) {
        self.inner
            .failures
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(point);
    }

    /// Whether a commit at `point` fails now; disarms the failure.
    #[must_use]
    pub fn take_failure(&self, point: CommitPoint) -> bool {
        self.inner
            .failures
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&point)
    }

    /// Every commit awaits `hook` at every point until [`Faults::clear_pause`].
    pub fn pause_with(&self, hook: PauseHook) {
        *self
            .inner
            .pause
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(hook);
    }

    /// Stops pausing commits.
    pub fn clear_pause(&self) {
        *self
            .inner
            .pause
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
    }

    /// The wait a commit at `point` must await, if a pause is armed.
    #[must_use]
    pub fn pause_at(&self, point: CommitPoint) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
        let hook = self
            .inner
            .pause
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        hook.map(|hook| hook(point))
    }
}

impl std::fmt::Debug for Faults {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Faults").finish_non_exhaustive()
    }
}
