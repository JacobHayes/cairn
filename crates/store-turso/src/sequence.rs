//! Log positions in commit-visible order. Turso numbers a row when it is inserted, not when
//! its transaction commits, so with commits to different domains in flight together a
//! later position can become visible before an earlier one, and a reader paging past it
//! would skip the earlier one for good. The store numbers log rows itself, from one
//! sequencer per log in this process (one process per database, ARCHITECTURE Assumptions),
//! and a reader only reads below the lowest position a transaction in flight still holds:
//! below it, every position's transaction has ended before the read began.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

/// Hands out positions and knows which are held by transactions in flight.
#[derive(Debug)]
pub(crate) struct Sequencer {
    state: Arc<Mutex<State>>,
}

#[derive(Debug)]
struct State {
    next: i64,
    /// The first position of each block in flight, with how many blocks start there.
    held: BTreeMap<i64, usize>,
}

/// Positions held by one transaction in flight, released when dropped (after the
/// transaction commits or rolls back, or when its future is dropped).
#[derive(Debug)]
pub(crate) struct Reservation {
    first: i64,
    state: Arc<Mutex<State>>,
}

impl Sequencer {
    /// A sequencer whose next position is `next`.
    pub fn new(next: i64) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                next,
                held: BTreeMap::new(),
            })),
        }
    }

    /// Holds `count` positions, from the returned one up.
    pub fn reserve(&self, count: usize) -> Reservation {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let first = state.next;
        state.next = first.saturating_add(i64::try_from(count).unwrap_or(i64::MAX));
        *state.held.entry(first).or_default() += 1;
        Reservation {
            first,
            state: Arc::clone(&self.state),
        }
    }

    /// The position a reader reads below: the lowest one in flight, or the next to be
    /// handed out. Taken before the reader's transaction begins.
    pub fn horizon(&self) -> i64 {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.held.keys().next().copied().unwrap_or(state.next)
    }
}

impl Reservation {
    /// The first position held.
    pub fn first(&self) -> i64 {
        self.first
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(count) = state.held.get_mut(&self.first) {
            *count -= 1;
            if *count == 0 {
                state.held.remove(&self.first);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readers_stop_below_the_lowest_position_in_flight() {
        let sequencer = Sequencer::new(1);
        assert_eq!(sequencer.horizon(), 1);
        let early = sequencer.reserve(2);
        let late = sequencer.reserve(3);
        assert_eq!((early.first(), late.first()), (1, 3));
        drop(late);
        assert_eq!(sequencer.horizon(), 1);
        drop(early);
        assert_eq!(sequencer.horizon(), 6);
    }
}
