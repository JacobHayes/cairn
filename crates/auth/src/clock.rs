//! The clock the auth crate reads, a seam so tests set the time (PRACTICES, Logical
//! interfaces: the clock is an input).

use std::sync::Arc;
use std::time::Duration;

use cairn_schema::Timestamp;

/// Now, as the auth crate sees it.
#[derive(Clone)]
pub struct Clock(Arc<dyn Fn() -> Timestamp + Send + Sync>);

impl Clock {
    /// The system clock.
    #[must_use]
    pub fn system() -> Self {
        Self(Arc::new(Timestamp::now))
    }

    /// A clock that reads `now`.
    #[must_use]
    pub fn from_fn(now: impl Fn() -> Timestamp + Send + Sync + 'static) -> Self {
        Self(Arc::new(now))
    }

    /// The current time.
    #[must_use]
    pub fn now(&self) -> Timestamp {
        (self.0)()
    }

    /// The time `lifetime` from now.
    ///
    /// # Panics
    ///
    /// When that time is past what a timestamp holds: the lifetimes are days, not ages.
    #[must_use]
    pub fn after(&self, lifetime: Duration) -> Timestamp {
        let now = self.now();
        let later = jiff::SignedDuration::try_from(lifetime)
            .ok()
            .and_then(|lifetime| now.checked_add(lifetime).ok());
        assert!(
            later.is_some(),
            "a lifetime from now is a representable time"
        );
        later.unwrap_or(now)
    }
}

impl std::fmt::Debug for Clock {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("Clock").field(&self.now()).finish()
    }
}
