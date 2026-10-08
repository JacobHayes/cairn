//! The API's own request and response bodies (ARCHITECTURE, HTTP API: Rust types are the
//! source of truth for the OpenAPI document). Domain values (patches, journeys, events,
//! rejections) cross unchanged as the schema crate's types; these wrap them, and give the
//! service's answers and the store's query results a wire shape.

mod assistant;
mod projections;
mod proposals;
mod reads;
mod users;

pub use assistant::{
    AssistantRequest, Conversation, ConversationAuthor, ConversationEntry, TurnReply,
};
pub use projections::{ChildEntry, History, Mine, NodeDetail, Projected};
pub use proposals::{
    ProposalAnswer, ProposalApply, ProposalCreate, ProposalEdit, ProposalReview, ProposalStep,
    RelinkRequest, RouteImport, SaveAsRouteRequest, StaleBase, UpgradeRequest,
};
pub use reads::{
    EventPage, JourneyMatches, JourneyPage, JourneySummary, LoggedEvent, RouteDetail, RoutePage,
    RouteSummary, SearchHit, SearchPage, VersionJourneys,
};
pub use users::{AgentToken, LinkedIdentity, MintedToken, TokenRequest, Viewer};

use std::collections::BTreeMap;

use cairn_schema::{
    Consequences, JourneyId, Markdown, Patch, PatchReceipt, Revision, RevisionOf, Slug,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `POST /{domain}/patches`: a patch to the domain the path names, and the note each of its
/// events carries (J1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PatchRequest {
    /// The patch; its target must be the path's domain.
    pub patch: Patch,
    /// A note for each of its events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<Markdown>,
}

/// What an accepted patch answers (A17, H5, D7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum PatchAnswer {
    /// Committed now.
    Applied {
        /// The receipt: patch id, domain, content hash, and the revision produced.
        receipt: PatchReceipt,
        /// D7: what it newly caused, by journey; a journey with nothing new is left out.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        consequences: BTreeMap<JourneyId, Consequences>,
    },
    /// The patch id was committed before with the same content (H5): its receipt, and no
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

impl From<cairn_service::Written> for PatchAnswer {
    fn from(written: cairn_service::Written) -> Self {
        match written {
            cairn_service::Written::Applied {
                receipt,
                consequences,
            } => PatchAnswer::Applied {
                receipt,
                consequences,
            },
            cairn_service::Written::AlreadyApplied { receipt } => {
                PatchAnswer::AlreadyApplied { receipt }
            }
        }
    }
}

/// The capabilities document (`GET /capabilities`): what this host offers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    /// How people sign in, in the order the host asks.
    pub auth: Vec<AuthMethod>,
    /// Whether the in-app assistant is available (I5).
    pub assistant: bool,
    /// Whether the host serves MCP at `/mcp` (I2).
    pub mcp: bool,
    /// Whether the host streams revision ticks over SSE (H6).
    pub sse: bool,
}

/// One way to sign in (H1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthMethod {
    /// The provider's configured name.
    pub name: Slug,
    /// What kind of provider it is.
    pub kind: AuthKind,
}

/// The kinds of sign-in a host can offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    /// A static token or named dev user (H1).
    Dev,
    /// OAuth or OIDC against a registered provider.
    Oidc,
    /// The built-in OAuth server for MCP clients (I2).
    BuiltinOauth,
    /// Tailscale identity.
    Tailscale,
    /// The browser host's single local identity.
    Local,
}

impl From<&cairn_service::Capabilities> for Capabilities {
    fn from(capabilities: &cairn_service::Capabilities) -> Self {
        let cairn_service::Capabilities {
            auth,
            assistant,
            mcp,
            sse,
        } = capabilities;
        let auth = auth.iter().map(|method| AuthMethod {
            name: method.name.clone(),
            kind: match method.kind {
                cairn_service::AuthKind::Dev => AuthKind::Dev,
                cairn_service::AuthKind::Oidc => AuthKind::Oidc,
                cairn_service::AuthKind::BuiltinOauth => AuthKind::BuiltinOauth,
                cairn_service::AuthKind::Tailscale => AuthKind::Tailscale,
                cairn_service::AuthKind::Local => AuthKind::Local,
            },
        });
        Self {
            auth: auth.collect(),
            assistant: *assistant,
            mcp: *mcp,
            sse: *sse,
        }
    }
}

/// One SSE `tick` event (H6): `of` is now at `revision`. A view refetches only when it is
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

/// The kinds of error that are not patch rejections (PRACTICES, Errors, panics, and
/// rejections). A rejected patch answers its `Rejection` instead.
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
    /// Cairn cannot draft the upgrade, save as route, or re-link asked for: the journey follows
    /// no route, the version is not newer, or the draft is past a proposal's limits (B7, B8,
    /// B9).
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
    /// Only a user may do this, not an agent acting for one (H2).
    UserOnly,
    /// The process is at its in-flight request limit; retry after the `Retry-After` delay.
    Overloaded,
    /// The process is at its SSE subscriber limit; retry after the `Retry-After` delay.
    SubscriberLimit,
    /// No store connection came free in time; retry after the `Retry-After` delay.
    StoreBusy,
    /// The request ran past the request duration limit. A patch may have committed:
    /// resubmit it with the same patch id (H5).
    TimedOut,
    /// The engine panicked: a bug. Nothing was committed.
    EnginePanic,
    /// The store or another part of the server failed.
    Internal,
}
