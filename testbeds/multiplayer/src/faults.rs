//! The testbed's fault sites inside the server, for what the network cannot do: make two
//! commits to one domain overlap. One server task on one thread runs each request to its
//! end unless something waits, and a commit takes no virtual time, so without these no two
//! commits ever meet. They hang on the store's own commit points (`cairn_store::Faults`).
//! Every site is a `buggify!`, inert without `--buggify`, and none fires during setup.
//!
//! - `store-commit-waits` (short) and `store-commit-waits-past-timeout` (a few per run): a
//!   commit waits before it begins, as one does for a pooled connection. Meanwhile another
//!   commit that loaded the same revision can land first, so this one loses at commit; or,
//!   past the caller's attempt timeout, the caller resubmits and the resubmission lands
//!   first, so the original meets its own receipt (H5).
//! - `store-commit-stalls` (short) and `store-commit-stalls-past-timeout` (once per run): a
//!   commit holds its transaction open just before it commits, as a slow write does. Every
//!   other commit to the domain conflicts with it in the meantime.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use cairn_store::{CommitPoint, Faults};

use crate::client::ATTEMPT_TIMEOUT;

/// A short wait or stall: long enough for another request to load and commit.
const SHORT: Duration = Duration::from_millis(5);
/// A long one: past the client's attempt timeout, so the caller resubmits meanwhile.
const LONG: Duration = Duration::from_millis(400);
/// Long waits per run: a long wait on an HTTP request only times its attempt out (the
/// server drops a request whose client has gone), so it takes a few to land on the agent.
const LONG_WAITS_PER_RUN: u32 = 4;
/// Long stalls per run: one holds up every other commit to the domain, which is enough.
const LONG_STALLS_PER_RUN: u32 = 1;

/// The long faults outlast an attempt, which is what makes the caller resubmit.
const _: () = assert!(LONG.as_millis() > ATTEMPT_TIMEOUT.as_millis());

/// Whether the sites may fire, and how many long ones are left.
#[derive(Debug, Default)]
pub struct Sites {
    armed: AtomicBool,
    long_waits: AtomicU32,
    long_stalls: AtomicU32,
}

impl Sites {
    /// The sites, unarmed, and the store faults that consult them at every commit point.
    pub fn hooked() -> (Arc<Sites>, Faults) {
        let sites = Arc::new(Sites::default());
        let faults = Faults::default();
        let hook = Arc::clone(&sites);
        faults.pause_with(Arc::new(move |point| Arc::clone(&hook).pause(point)));
        (sites, faults)
    }

    /// Lets the sites fire from now on: setup is done.
    pub fn arm(&self) {
        self.long_waits.store(LONG_WAITS_PER_RUN, Ordering::SeqCst);
        self.long_stalls
            .store(LONG_STALLS_PER_RUN, Ordering::SeqCst);
        self.armed.store(true, Ordering::SeqCst);
    }

    /// What a commit at `point` waits on.
    fn pause(self: Arc<Self>, point: CommitPoint) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async move {
            if !self.armed.load(Ordering::SeqCst) {
                return;
            }
            let pause = match point {
                CommitPoint::BeforeBegin => {
                    if patina_dst::buggify!("store-commit-waits-past-timeout")
                        && take(&self.long_waits)
                    {
                        LONG
                    } else if patina_dst::buggify!("store-commit-waits") {
                        SHORT
                    } else {
                        return;
                    }
                }
                CommitPoint::BeforeCommit => {
                    if patina_dst::buggify!("store-commit-stalls-past-timeout")
                        && take(&self.long_stalls)
                    {
                        LONG
                    } else if patina_dst::buggify!("store-commit-stalls") {
                        SHORT
                    } else {
                        return;
                    }
                }
                CommitPoint::AfterRevisionRow | CommitPoint::BetweenStateAndEvents => return,
            };
            tokio::time::sleep(pause).await;
        })
    }
}

/// Takes one long fault from `budget`, if any is left.
fn take(budget: &AtomicU32) -> bool {
    budget
        .try_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
            left.checked_sub(1)
        })
        .is_ok()
}
