//! Commits queue in process on what they write (ARCHITECTURE, Concurrency and notification;
//! decisions/2026-10-06-a-resubmission-beside-its-own-original-in-flight-is-answered.md):
//! before it begins, a commit takes its turn at every revision row it writes and at its
//! patch id, and holds them until its transaction ends. A commit beside one in flight on
//! the same rows therefore waits for it and then sees what it did: a resubmission meets its
//! original's receipt, and a stale commit is answered with what really intervened, never
//! with a revision still in flight (H5). Commits to different domains wait on each other
//! only at a row both write: every commit that writes an entity reference or creates an
//! entity claims the deployment's revision row, so those queue across journeys and routes.
//! Turns are taken in one order (the claims' order), so no commit waits on one that waits
//! on it.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};

use cairn_schema::{PatchId, RevisionOf};
use cairn_store::StoreError;
use cairn_store::limits::STORE_CONNECTION_ACQUIRE;
use tokio::sync::OwnedMutexGuard;

/// What a commit writes that another commit in flight may write too.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Claim {
    /// A domain's or proposal's revision row.
    Revision(RevisionOf),
    /// A patch id's receipt.
    Patch(PatchId),
}

/// One turn per claim held or waited for; a claim nobody holds or waits for is forgotten,
/// so the map is never larger than what is in flight.
#[derive(Default)]
pub(crate) struct Queues {
    turns: Mutex<BTreeMap<Claim, Arc<tokio::sync::Mutex<()>>>>,
}

/// A wait for one claim's turn. However it ends (taken, timed out, or dropped with its
/// commit, even after the turn was handed to it), it lets go of the turn's reference and then
/// forgets the claim if nobody else holds or waits for it.
struct Waiting<'queues> {
    queues: &'queues Queues,
    claim: Claim,
    lock: Option<Pin<Box<dyn Future<Output = OwnedMutexGuard<()>> + Send>>>,
}

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        self.lock = None;
        self.queues.forget_if_free(&self.claim);
    }
}

/// The turns a commit holds, given back when it is dropped.
pub(crate) struct Held<'queues> {
    queues: &'queues Queues,
    turns: Vec<(Claim, OwnedMutexGuard<()>)>,
}

impl Queues {
    /// Waits for a turn at every claim, in order, at most `store_connection_acquire` in all
    /// (PRACTICES, Explicit limits: a wait that long is contention, which at this scale is a
    /// bug).
    ///
    /// # Errors
    ///
    /// [`StoreError::AcquireTimeout`] when the turns did not come in time.
    pub(crate) async fn take(&self, claims: BTreeSet<Claim>) -> Result<Held<'_>, StoreError> {
        let deadline = tokio::time::Instant::now() + STORE_CONNECTION_ACQUIRE;
        let mut held = Held {
            queues: self,
            turns: Vec::with_capacity(claims.len()),
        };
        for claim in claims {
            let turn = self
                .turns
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entry(claim.clone())
                .or_default()
                .clone();
            let mut waiting = Waiting {
                queues: self,
                claim,
                lock: Some(Box::pin(turn.lock_owned())),
            };
            let Some(lock) = waiting.lock.as_mut() else {
                unreachable!("a wait holds its lock until it is dropped")
            };
            let Ok(guard) = tokio::time::timeout_at(deadline, lock.as_mut()).await else {
                return Err(StoreError::AcquireTimeout);
            };
            held.turns.push((waiting.claim.clone(), guard));
        }
        Ok(held)
    }

    /// Forgets `claim` when nobody holds or waits for its turn: the map's is the only
    /// reference left, and nobody can take another without the map's lock.
    fn forget_if_free(&self, claim: &Claim) {
        let mut turns = self.turns.lock().unwrap_or_else(PoisonError::into_inner);
        if turns
            .get(claim)
            .is_some_and(|turn| Arc::strong_count(turn) == 1)
        {
            turns.remove(claim);
        }
    }
}

impl Drop for Held<'_> {
    fn drop(&mut self) {
        for (claim, guard) in self.turns.drain(..) {
            drop(guard);
            self.queues.forget_if_free(&claim);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::pin;
    use std::task::Poll;

    fn run<F: Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .start_paused(true)
            .build()
            .unwrap()
            .block_on(future)
    }

    fn claims(ids: &[&str]) -> BTreeSet<Claim> {
        ids.iter()
            .map(|id| Claim::Patch(id.parse().unwrap()))
            .collect()
    }

    fn remembered(queues: &Queues) -> usize {
        queues.turns.lock().unwrap().len()
    }

    /// Whether `future` is still waiting after one poll.
    async fn waits<F: Future + Unpin>(future: &mut F) -> bool {
        std::future::poll_fn(|context| {
            Poll::Ready(std::pin::Pin::new(&mut *future).poll(context).is_pending())
        })
        .await
    }

    #[test]
    fn a_claim_held_makes_the_next_commit_wait_until_it_is_given_back() {
        run(async {
            let queues = Queues::default();
            let first = queues.take(claims(&["p_a", "p_b"])).await.unwrap();
            let other = queues.take(claims(&["p_c"])).await;
            assert!(other.is_ok(), "a different claim does not wait");
            drop(other);
            let mut waiting = pin!(queues.take(claims(&["p_b"])));
            assert!(waits(&mut waiting).await, "a held claim waits");
            drop(first);
            assert!(waiting.await.is_ok());
            assert_eq!(remembered(&queues), 0, "nothing held, nothing remembered");
        });
    }

    #[test]
    fn a_waiter_dropped_after_its_turn_was_handed_over_leaves_nothing() {
        run(async {
            let queues = Queues::default();
            let first = queues.take(claims(&["p_a"])).await.unwrap();
            let mut waiting = Box::pin(queues.take(claims(&["p_a"])));
            assert!(waits(&mut waiting).await, "a held claim waits");
            drop(first);
            drop(waiting);
            assert_eq!(remembered(&queues), 0, "nothing held, nothing remembered");
        });
    }

    #[test]
    fn a_turn_not_given_back_within_the_acquire_limit_times_out() {
        run(async {
            let queues = Queues::default();
            let held = queues.take(claims(&["p_a"])).await.unwrap();
            let started = tokio::time::Instant::now();
            let waited = queues.take(claims(&["p_a", "p_b"])).await;
            assert!(matches!(waited, Err(StoreError::AcquireTimeout)));
            assert_eq!(started.elapsed(), STORE_CONNECTION_ACQUIRE);
            drop(held);
            assert_eq!(
                remembered(&queues),
                0,
                "a wait that timed out leaves nothing"
            );
        });
    }
}
