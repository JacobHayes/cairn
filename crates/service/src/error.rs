//! What goes wrong in the service that is not an answer about a patch (PRACTICES, Errors,
//! panics, and rejections).

use std::fmt;

use cairn_schema::Domain;
use cairn_store::{StoreError, SubscriberLimit};

/// A failure of the service or what it stands on. A rejected patch is not one: writes answer
/// those as rejections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceError {
    /// The store failed.
    Store(StoreError),
    /// The process is at its subscriber limit (H6).
    SubscriberLimit(SubscriberLimit),
    /// The engine panicked working on `domain`: a bug. It worked on a private candidate, so
    /// nothing was committed; the host answers a server error and logs it with the domain
    /// (PRACTICES, Programmer errors panic).
    EnginePanic {
        /// The domain the call was about.
        domain: Domain,
        /// The panic's message.
        message: String,
    },
}

impl From<StoreError> for ServiceError {
    fn from(error: StoreError) -> Self {
        ServiceError::Store(error)
    }
}

impl From<SubscriberLimit> for ServiceError {
    fn from(error: SubscriberLimit) -> Self {
        ServiceError::SubscriberLimit(error)
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceError::Store(error) => error.fmt(formatter),
            ServiceError::SubscriberLimit(error) => error.fmt(formatter),
            ServiceError::EnginePanic { domain, message } => {
                write!(formatter, "the engine panicked on {domain}: {message}")
            }
        }
    }
}

impl std::error::Error for ServiceError {}
