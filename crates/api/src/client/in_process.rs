//! H5's safe retry against the service itself, with no transport: what a caller in the
//! same process (the multiplayer testbed, 6.1) submits patches with.

use cairn_schema::{Markdown, Patch};
use cairn_service::{Call, DomainPatch, Service, ServiceError, WriteError};
use cairn_store::Store;

use super::retry::{self, Landed, Refused};

/// Why a patch could not reach the service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InProcessError {
    /// The service failed.
    Service(ServiceError),
    /// The patch targets a proposal.
    NotADomainPatch,
}

/// H5: submits `patch` to `service` as `call`, resubmitting it on its own while it is
/// stale and safe to retry, exactly as the HTTP client does.
///
/// # Errors
///
/// Its rejection (a stale one when retrying was not safe), or the service's failure.
pub async fn patch<S: Store>(
    service: &Service<S>,
    call: &Call,
    patch: Patch,
    note: Option<Markdown>,
) -> Result<Landed, Refused<InProcessError>> {
    retry::submit(patch, |patch| {
        let note = note.clone();
        async move {
            let submitted = DomainPatch::new(patch, note)
                .map_err(|_| Refused::Failed(InProcessError::NotADomainPatch))?;
            match service.patch(call, &submitted).await {
                Ok(written) => Ok(written.into()),
                Err(WriteError::Rejected(rejection)) => Err(Refused::Rejected(rejection)),
                Err(WriteError::Failed(error)) => {
                    Err(Refused::Failed(InProcessError::Service(error)))
                }
            }
        }
    })
    .await
}
