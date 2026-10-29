//! The request and response bodies every host speaks (ARCHITECTURE, HTTP API: Rust types are
//! the source of truth for the OpenAPI document). Domain values (patches, journeys, events,
//! rejections) cross unchanged as the schema crate's types; these wrap them, and give the
//! service's answers and the store's query results a wire shape. The HTTP API and the
//! cairn demo both serialize these types, so a body is the same bytes from either.

mod projections;
mod proposals;
mod reads;
mod users;

pub use crate::{AgentSignIn, AuthKind, AuthMethod, Capabilities, McpAccess};
pub use projections::{ChildEntry, History, Mine, NodeDetail, Projected};
pub use proposals::{
    DestinationKind, DestinationNow, ProposalAnswer, ProposalApply, ProposalCreate, ProposalCursor,
    ProposalEdit, ProposalList, ProposalReview, ProposalStep, ProposalSummary, RouteImport,
    SaveAsRouteRequest, StaleBase,
};
pub use reads::{
    EventPage, IdentityReferences, InsertionUse, JourneyMatches, JourneyPage, JourneySummary,
    LoggedEvent, RouteDetail, RoutePage, RouteReference, RouteSummary, SearchHit, SearchPage,
    VersionJourneys,
};
pub use users::{
    Caller, LinkedLogin, Tour, TourExpect, TourStanding, TourWrite, TourWriteState, UserName,
};

use std::collections::BTreeMap;

use cairn_schema::{
    Consequences, JourneyId, Markdown, Notice, Patch, PatchReceipt, Revision, RevisionOf,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// PRD: J1.
/// `POST /api/{domain}/patches`: a patch to the domain the path names, and the note each of its
/// events carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PatchRequest {
    /// The patch; its target must be the path's domain.
    pub patch: Patch,
    /// A note for each of its events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<Markdown>,
}

// PRD: A17, H5, D7.
/// What an accepted patch answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum PatchAnswer {
    /// Committed now.
    Applied {
        /// The receipt: patch id, domain, content hash, and the revision produced.
        receipt: PatchReceipt,
        // PRD: D7.
        /// What it newly caused, by journey; a journey with nothing new is left out.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        consequences: BTreeMap<JourneyId, Consequences>,
        // PRD: A20.
        /// Advisory notices about the route graph an import or publish leaves; never a
        /// rejection, and absent for any other patch.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        notices: Vec<Notice>,
    },
    // PRD: H5.
    /// The patch id was committed before with the same content: its receipt, and no
    /// consequences, which are reported only the first time.
    AlreadyApplied {
        /// The original receipt.
        receipt: PatchReceipt,
    },
}

impl PatchAnswer {
    /// The receipt, whichever way it went.
    #[must_use]
    pub fn receipt(&self) -> &PatchReceipt {
        match self {
            PatchAnswer::Applied { receipt, .. } | PatchAnswer::AlreadyApplied { receipt } => {
                receipt
            }
        }
    }
}

impl From<crate::Written> for PatchAnswer {
    fn from(written: crate::Written) -> Self {
        match written {
            crate::Written::Applied {
                receipt,
                consequences,
                notices,
            } => PatchAnswer::Applied {
                receipt,
                consequences,
                notices,
            },
            crate::Written::AlreadyApplied { receipt } => PatchAnswer::AlreadyApplied { receipt },
        }
    }
}

/// `GET /healthz`: whether the server is serving. Answered 200 when it is, and 503 when its
/// store has failed closed and answers nothing until the process is restarted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Health {
    /// Serving, or why not.
    pub status: HealthStatus,
}

/// Whether the server is serving.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// Serving.
    Ok,
    /// The store has failed closed: every call errs until the process is restarted.
    StoreFailedClosed,
}

// PRD: H6.
/// One SSE `tick` event: `of` is now at `revision`. A view refetches only when it is
/// newer than the revision it holds.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Tick {
    /// The domain or proposal.
    pub of: RevisionOf,
    /// Its revision; 0 for one that does not exist.
    pub revision: Revision,
}

impl From<cairn_store::Tick> for Tick {
    fn from(tick: cairn_store::Tick) -> Self {
        Self {
            of: tick.of,
            revision: tick.revision,
        }
    }
}

/// An error that is not a patch rejection: what went wrong, for a person to read, and the
/// request id the server logged it under.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Problem {
    /// What kind of error it is.
    pub error: ProblemCode,
    /// What went wrong.
    pub message: String,
    /// The request's id, as the server's logs name it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

// PRACTICES: Errors, panics, and rejections.
/// The kinds of error that are not patch rejections. A rejected patch answers its `Rejection`
/// instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProblemCode {
    /// The request is malformed: a body, path segment, or query parameter that does not
    /// parse, or an unknown query parameter.
    BadRequest,
    /// A body that is not `application/json`.
    UnsupportedMediaType,
    /// A body over the request size limit.
    PayloadTooLarge,
    /// The patch's target is not the domain the path names, or is a proposal.
    TargetMismatch,
    // PRD: B8.
    /// Cairn cannot draft the save as route asked for: the draft is past a proposal's limits.
    CannotDraft,
    /// No such resource.
    NotFound,
    /// The journey moved since the page a cursor came from was read, so the cursor no longer
    /// names a place in its order: start again from the first page.
    PageMoved,
    /// No endpoint at this path.
    NoSuchEndpoint,
    /// The endpoint takes another method.
    MethodNotAllowed,
    // PRD: H2.
    /// Only a user may do this, not an agent acting for one.
    UserOnly,
    // PRD: H2.
    /// This deployment has no sign-in that accepts agent tokens, so one cannot be made.
    TokensNotAccepted,
    /// The process is at its in-flight request limit; retry after the `Retry-After` delay.
    Overloaded,
    /// A reply in this assistant conversation is still in progress: wait for it, then send
    /// again.
    TurnInProgress,
    /// The process is at its SSE subscriber limit; retry after the `Retry-After` delay.
    SubscriberLimit,
    /// No store connection came free in time; retry after the `Retry-After` delay.
    StoreBusy,
    // PRD: H5.
    /// The request ran past the request duration limit. A patch may have committed:
    /// resubmit it with the same patch id.
    TimedOut,
    /// The engine panicked: a bug. Nothing was committed.
    EnginePanic,
    /// The store or another part of the server failed.
    Internal,
}
