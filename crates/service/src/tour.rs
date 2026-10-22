//! A person's place in the tour of Cairn (PRD: C20): read with the deployment's today,
//! and written only over the record the writer expected.

use cairn_schema::{Date, JourneyId};
use cairn_store::{TourMark, TourRecord, TourState};

use crate::{Call, Service, ServiceError};

/// The caller's record, if they have one, and the deployment's today.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TourView {
    /// The record; none until they first start or dismiss the tour.
    pub record: Option<TourRecord>,
    /// Today in the deployment's zone: the date the practice journey's screens call today.
    pub today: Date,
}

impl Service {
    // PRD: C20.
    /// The caller's place in the tour, and today.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn tour(&self, call: &Call) -> Result<TourView, ServiceError> {
        Ok(TourView {
            record: self.store.tour(&call.actor.user).await?,
            today: self.settings.today(call.now),
        })
    }

    // PRD: C20.
    /// Moves the caller's tour to `state` (with `practice_journey` while it is started) only
    /// when their record is `expect` now (`None` for no record); on a mismatch, the current
    /// record, unchanged.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn put_tour(
        &self,
        call: &Call,
        state: TourState,
        practice_journey: Option<JourneyId>,
        expect: Option<TourMark>,
    ) -> Result<Result<(), Option<TourRecord>>, ServiceError> {
        let record = TourRecord {
            user: call.actor.user.clone(),
            state,
            practice_journey,
            updated_at: call.now,
        };
        Ok(self.store.put_tour(record, expect).await?)
    }
}
