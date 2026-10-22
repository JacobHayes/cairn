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
    DestinationKind, DestinationNow, ProposalAnswer, ProposalApply, ProposalCreate, ProposalCursor,
    ProposalEdit, ProposalList, ProposalReview, ProposalStep, ProposalSummary, RelinkRequest,
    RouteImport, SaveAsRouteRequest, StaleBase, UpgradeRequest,
};
pub use reads::{
    EventPage, IdentityReferences, InsertionUse, JourneyMatches, JourneyPage, JourneySummary,
    LoggedEvent, RouteDetail, RoutePage, RouteReference, RouteSummary, SearchHit, SearchPage,
    VersionJourneys,
};
pub use users::{AgentToken, Caller, LinkedLogin, MintedToken, TokenRequest, UserName};

use std::collections::BTreeMap;

use cairn_schema::{
    Consequences, JourneyId, Markdown, Notice, Patch, PatchReceipt, Revision, RevisionOf, Slug,
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

impl From<cairn_service::Written> for PatchAnswer {
    fn from(written: cairn_service::Written) -> Self {
        match written {
            cairn_service::Written::Applied {
                receipt,
                consequences,
                notices,
            } => PatchAnswer::Applied {
                receipt,
                consequences,
                notices,
            },
            cairn_service::Written::AlreadyApplied { receipt } => {
                PatchAnswer::AlreadyApplied { receipt }
            }
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

/// The capabilities document (`GET /api/capabilities`): what this host offers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    /// How people sign in, in the order the host asks.
    pub auth: Vec<AuthMethod>,
    // PRD: I5.
    /// Whether the in-app assistant is available.
    pub assistant: bool,
    // PRD: I2.
    /// How an agent reaches this host's MCP server; null where the host serves none.
    pub mcp: Option<McpAccess>,
    // PRD: H6.
    /// Whether the host streams revision ticks over SSE.
    pub sse: bool,
}

/// How an agent reaches this host's MCP server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct McpAccess {
    /// The MCP endpoint's absolute address: the public URL joined with the MCP path.
    pub url: String,
    /// How an agent signs in, in the order the providers are asked; empty when it cannot.
    pub sign_in: Vec<AgentSignIn>,
    /// Whether a person can make agent tokens here, and list and revoke theirs.
    pub tokens: bool,
}

/// One way an agent signs in to the MCP server.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentSignIn {
    /// The built-in OAuth server: a browser step on first use; the agent is recorded beside
    /// the person.
    Oauth,
    /// Behind IAP, the relay to Google: a browser step with Google, the client signing in on
    /// the machine it runs on.
    GoogleOauth,
    /// Tailscale, either mode: no sign-in step, from a machine in the tailnet signed in as a
    /// person.
    Tailnet,
    /// The dev provider without a token: every request it accepts is its dev user.
    DevUser,
}

// PRD: H1.
/// One way to sign in.
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
    // PRD: H1.
    /// A static token or named dev user.
    Dev,
    /// OAuth or OIDC against a registered provider.
    Oidc,
    // PRD: I2.
    /// The built-in OAuth server for MCP clients.
    BuiltinOauth,
    /// Tailscale login.
    Tailscale,
    /// Google Cloud Identity-Aware Proxy's signed assertion.
    GcpIap,
    /// The browser host's single local login.
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
                cairn_service::AuthKind::GcpIap => AuthKind::GcpIap,
                cairn_service::AuthKind::Local => AuthKind::Local,
            },
        });
        Self {
            auth: auth.collect(),
            assistant: *assistant,
            mcp: mcp.as_ref().map(|access| McpAccess {
                url: access.url.clone(),
                sign_in: access
                    .sign_in
                    .iter()
                    .map(|how| match how {
                        cairn_service::AgentSignIn::Oauth => AgentSignIn::Oauth,
                        cairn_service::AgentSignIn::GoogleOauth => AgentSignIn::GoogleOauth,
                        cairn_service::AgentSignIn::Tailnet => AgentSignIn::Tailnet,
                        cairn_service::AgentSignIn::DevUser => AgentSignIn::DevUser,
                    })
                    .collect(),
                tokens: access.tokens,
            }),
            sse: *sse,
        }
    }
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
    // PRD: B7, B8, B9.
    /// Cairn cannot draft the upgrade, save as route, or re-link asked for: the journey follows
    /// no route, the version is not newer, or the draft is past a proposal's limits.
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
