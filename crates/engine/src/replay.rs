//! Replay (J3): records rebuilt from a starting point and the events after it. Each event's
//! delta is the after-state of what its mutation wrote (J1), so replay applies the writes in
//! order and advances revisions once per patch, re-deriving nothing; it shares
//! [`Records::write`] and [`Records::advance`] with apply, so the rebuilt records equal the
//! ones apply produced.
//!
//! Cost: one clone of the starting records and one write per delta entry; at most 8,000
//! events per patch.

use cairn_schema::Event;

use crate::records::Records;

/// J3: the records `events` produce from `initial`, patch by patch.
///
/// # Panics
///
/// When the events are not whole patches in order: each patch's events share its id and
/// run from ordinal 0 without a gap.
#[must_use]
pub fn replay(initial: &Records, events: &[Event]) -> Records {
    let mut records = initial.clone();
    let mut rest = events;
    while let Some(first) = rest.first() {
        assert_eq!(
            first.ordinal, 0,
            "replay starts each patch at its first event"
        );
        let length = 1 + rest
            .iter()
            .skip(1)
            .take_while(|event| event.patch_id == first.patch_id && event.ordinal != 0)
            .count();
        let (patch, later) = rest.split_at(length);
        for (ordinal, event) in patch.iter().enumerate() {
            assert_eq!(
                event.ordinal as usize, ordinal,
                "a patch's events are in order"
            );
            for write in &event.delta {
                records.write(write);
            }
        }
        records.advance(patch);
        rest = later;
    }
    records
}
