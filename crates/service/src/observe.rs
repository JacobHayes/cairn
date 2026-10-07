//! What the service tells the host's observability (ARCHITECTURE, Observability; PRACTICES,
//! Errors, panics, and rejections): how long each derive takes, through the `metrics` facade
//! to whatever exporter the host installs, and whether the current thread is inside an
//! engine call, so the host's panic hook can tell a panic the service catches from one that
//! must stop the process.

use std::cell::Cell;

/// Derive latency in seconds: the read path's derive and each side of a write's
/// consequences.
pub const DERIVE_DURATION: &str = "cairn_derive_duration_seconds";

thread_local! {
    /// How many engine calls this thread is inside (they never nest today; a count keeps the
    /// flag right if one ever does).
    static ENGINE_CALL_DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Whether the current thread is inside an engine call the service catches panics from
/// (PRACTICES: the service layer catches a panic from an engine call inside a request; a
/// panic anywhere else stops the process). A host's panic hook reads it.
#[must_use]
pub fn engine_call_active() -> bool {
    ENGINE_CALL_DEPTH.with(Cell::get) > 0
}

/// Marks the current thread inside an engine call until it is dropped, on return or on
/// unwind alike.
pub(crate) struct EngineCall(());

impl EngineCall {
    pub(crate) fn enter() -> Self {
        ENGINE_CALL_DEPTH.with(|depth| {
            let Some(entered) = depth.get().checked_add(1) else {
                unreachable!("engine calls nest a bounded number of times");
            };
            depth.set(entered);
        });
        Self(())
    }
}

impl Drop for EngineCall {
    fn drop(&mut self) {
        ENGINE_CALL_DEPTH.with(|depth| {
            assert!(depth.get() > 0, "an engine call left is one entered");
            depth.set(depth.get() - 1);
        });
    }
}

/// Runs a derive, recording its duration on the server. The browser host (wasm32) has no
/// monotonic clock the standard library can read, and its metrics would go nowhere, so
/// there the derive runs untimed.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn timed_derive<T>(derive: impl FnOnce() -> T) -> T {
    let started = std::time::Instant::now();
    let derived = derive();
    metrics::histogram!(DERIVE_DURATION).record(started.elapsed().as_secs_f64());
    derived
}

/// Runs a derive (the browser host: untimed, see the server's version).
#[cfg(target_arch = "wasm32")]
pub(crate) fn timed_derive<T>(derive: impl FnOnce() -> T) -> T {
    derive()
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use super::*;

    #[test]
    fn the_flag_is_set_inside_an_engine_call_and_cleared_after_a_return_or_an_unwind() {
        assert!(!engine_call_active());
        {
            let _call = EngineCall::enter();
            assert!(engine_call_active());
        }
        assert!(!engine_call_active());
        let unwound = catch_unwind(AssertUnwindSafe(|| {
            let _call = EngineCall::enter();
            panic!("an engine assertion failed");
        }));
        assert!(unwound.is_err());
        assert!(!engine_call_active());
    }
}
