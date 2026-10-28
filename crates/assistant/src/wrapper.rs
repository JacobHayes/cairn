//! The write wrapper (ARCHITECTURE, Assistant; PRD I5, I7): every tool call the assistant
//! makes passes through it on its way to the MCP tool set, in process and as the user it
//! acts for. Reads pass through. A write's domain patch is worked out without writing
//! ([`ToolSet::drafted_patch`]) and decided by mutation kind and destination:
//!
//! - structural (PRD glossary, Structural change; every write to a route or its draft) is
//!   drafted as one proposal instead, against the same destination and revision;
//! - any other state change applies directly, as one patch, like any external agent's, and
//!   is reported with the nodes it touched ([`touched_nodes`]) and its consequences (D7).
//!
//! Each tool says what it writes ([`Writes`]), and the wrapper decides by that: the proposal
//! tools draft proposals already and pass through. Applying a proposal is the user's click
//! (I7), and importing a file is the user's too, so both are refused ([`refusal`]); so is a
//! patch tool whose patch the wrapper cannot see, which keeps a write tool added later from
//! slipping past it.

use std::collections::{BTreeMap, BTreeSet};

use cairn_mcp::{
    CreateProposal, ProposalCall, ProposalOutput, ToolError, ToolSet, WriteOutput, Writes,
};
use cairn_schema::{
    Actor, BoundedVec, ChangeClass, Consequences, Domain, Graph, JourneyId, Markdown, Mutation,
    NodeKey, Patch, PatchId, PatchTarget, ProposalDraft, ProposalId, Rejection, Revision, Title,
};
use cairn_service::DomainPatch;
use cairn_store::{Document, LoadTarget, Store};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub use crate::touched::touched_nodes;

// PRD: I7.
/// Why the assistant never runs a tool that writes `writes`, when it is the user's to do;
/// `None` for what the assistant may write.
#[must_use]
pub const fn refusal(writes: Writes) -> Option<&'static str> {
    match writes {
        Writes::Confirmation => {
            Some("applying a proposal is the user's click: tell them it is ready for review")
        }
        Writes::File => {
            Some("importing a file is the user's to do; draft what it holds as a proposal instead")
        }
        Writes::Nothing | Writes::Patch | Writes::Proposal => None,
    }
}

/// What the wrapper does with one tool call (I5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Policy {
    /// A read, or a name no tool has: passed through.
    Read,
    /// A state change: applied as one patch.
    Direct,
    /// A write drafted as one proposal instead, and why.
    Propose(Because),
    /// A proposal tool: passed through.
    Proposal,
    /// Not run, and why.
    Refused(String),
    /// A write that would be refused as it stands: answered with that rejection, not run.
    Rejected,
}

/// Why a write became a proposal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum Because {
    // PRD: glossary, Structural change.
    /// It changes structure, or writes a route or its draft.
    Structural,
    /// The assistant drafted it as a proposal itself.
    Asked,
}

// PRD: I5.
/// What the report of an applied write names: the patch, its domain, and the revision it
/// produced.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AppliedReceipt {
    /// The patch id.
    pub patch_id: PatchId,
    /// The domain it was committed to.
    pub domain: Domain,
    /// The revision it produced.
    pub revision: Revision,
}

/// A write the assistant made, as the turn reports it (every direct write is reported
/// with its consequences).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Action {
    /// Applied directly, as one patch.
    Applied {
        /// The tool.
        tool: String,
        /// Which patch, where, and the revision it produced.
        receipt: AppliedReceipt,
        // PRD: I5.
        /// The nodes it wrote, in key order (a direct change is reported with the nodes
        /// it affected, which the panel links to); none for a write to no journey's nodes.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        nodes: Vec<NodeKey>,
        // PRD: D7.
        /// What it newly caused in each journey it changed.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        consequences: BTreeMap<JourneyId, Consequences>,
    },
    /// A write whose patch id was applied before, with the same content: answered from that
    /// write's receipt, and not applied again.
    AlreadyApplied {
        /// The tool.
        tool: String,
        /// The earlier write's receipt.
        receipt: AppliedReceipt,
    },
    /// Drafted as a proposal for the user to review and apply.
    Proposed {
        /// The tool.
        tool: String,
        /// The proposal.
        proposal: ProposalId,
        // PRD: I6.
        /// Its destination, under which it is addressed.
        destination: Domain,
        /// Why it is a proposal.
        because: Because,
    },
    /// An open proposal discarded, at the assistant's hand.
    Discarded {
        /// The tool.
        tool: String,
        /// The proposal.
        proposal: ProposalId,
    },
}

impl Action {
    /// The proposal it drafted or discarded, if it is about one.
    #[must_use]
    pub fn proposal(&self) -> Option<&ProposalId> {
        match self {
            Action::Applied { .. } | Action::AlreadyApplied { .. } => None,
            Action::Proposed { proposal, .. } | Action::Discarded { proposal, .. } => {
                Some(proposal)
            }
        }
    }
}

/// What a tool call came to: what the model is told, and the write it made, if any.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    /// The tool's answer, or why it was not run.
    pub answer: Result<Value, ToolError>,
    /// The write, when one was made.
    pub action: Option<Action>,
}

/// The policy for the tool `name` with `arguments` as `actor`, with the domain patch a write
/// would submit, and for a direct write the nodes it touches: what applying it at the
/// current revision writes, read against its journey in `store`. A state write that would be
/// refused there is answered with that rejection.
///
/// # Errors
///
/// What working the patch out answers: arguments that do not match, or a failed read.
pub async fn decide(
    tools: &ToolSet,
    store: &dyn Store,
    actor: &Actor,
    (name, arguments): (&str, &Value),
) -> Result<Decision, ToolError> {
    let writes = ToolSet::definitions()
        .into_iter()
        .find(|definition| definition.name == name)
        .map_or(Writes::Nothing, |definition| definition.writes);
    if let Some(why) = refusal(writes) {
        return Ok(Decision::Refused(why.to_owned()));
    }
    match writes {
        Writes::Nothing => return Ok(Decision::Read),
        Writes::Proposal => return Ok(Decision::Proposal),
        Writes::Patch | Writes::File | Writes::Confirmation => {}
    }
    let Some(drafted) = tools.drafted_patch(name, arguments.clone()).await? else {
        return Ok(Decision::Refused(format!(
            "the assistant cannot tell what {name} would change, so it does not run it"
        )));
    };
    if drafted.patch().change_class() == ChangeClass::Structural {
        return Ok(Decision::Propose(Because::Structural, drafted));
    }
    let touched = match tools.touched(actor, drafted.patch()).await? {
        Ok(touched) => touched,
        Err(rejection) => return Ok(Decision::Rejected(rejection)),
    };
    let stored = stored_journey(store, drafted.patch()).await?;
    // A write to a whole domain or graph names no nodes to link to.
    let nodes = touched_nodes(drafted.patch(), &touched, stored.as_ref()).unwrap_or_default();
    Ok(Decision::Direct { drafted, nodes })
}

/// The journey `patch` targets, as stored; `None` for another domain or a journey that does
/// not exist yet.
async fn stored_journey(store: &dyn Store, patch: &Patch) -> Result<Option<Graph>, ToolError> {
    let PatchTarget::Journey(journey) = &patch.target else {
        return Ok(None);
    };
    let loaded = store.load(&LoadTarget::Journey(journey.clone())).await;
    let loaded = loaded.map_err(|error| ToolError::Failed {
        message: error.to_string(),
    })?;
    Ok(match loaded {
        Some(Document::Journey(journey)) => Some(journey.graph),
        _ => None,
    })
}

/// What [`decide`] settles for one call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Passed through.
    Read,
    /// Applied as one patch.
    Direct {
        /// The patch.
        drafted: DomainPatch,
        /// The nodes it touches.
        nodes: BTreeSet<NodeKey>,
    },
    /// Drafted as one proposal of this patch's mutations instead.
    Propose(Because, DomainPatch),
    /// A proposal tool: passed through.
    Proposal,
    /// Not run.
    Refused(String),
    /// Not run: applied as it stands, it would be refused with this.
    Rejected(Rejection),
}

impl Decision {
    /// The policy it applies.
    #[must_use]
    pub fn policy(&self) -> Policy {
        match self {
            Decision::Read => Policy::Read,
            Decision::Direct { .. } => Policy::Direct,
            Decision::Propose(because, _) => Policy::Propose(because.clone()),
            Decision::Proposal => Policy::Proposal,
            Decision::Refused(why) => Policy::Refused(why.clone()),
            Decision::Rejected(_) => Policy::Rejected,
        }
    }
}

/// Runs the tool call `name` with `arguments` for `actor` (the assistant acting for its
/// user, H2) through the policy.
pub async fn run(
    tools: &ToolSet,
    store: &dyn Store,
    actor: &Actor,
    (name, arguments): (&str, Value),
) -> Outcome {
    let decision = match decide(tools, store, actor, (name, &arguments)).await {
        Ok(decision) => decision,
        Err(error) => return not_run(error),
    };
    match decision {
        Decision::Refused(message) => not_run(ToolError::Refused { message }),
        Decision::Rejected(rejection) => not_run(ToolError::Rejected { rejection }),
        Decision::Propose(because, drafted) => propose(tools, actor, name, &drafted, because).await,
        Decision::Direct { drafted, nodes } => {
            let domain = drafted.patch().target.domain();
            let answer = tools.call(actor, name, arguments).await;
            let action = answer
                .as_ref()
                .ok()
                .and_then(|output| applied(name, output, &nodes, (&drafted.patch().id, &domain)));
            patina_dst::sometimes!(action.is_some(), "assistant-direct-write-applied");
            Outcome { answer, action }
        }
        Decision::Proposal => {
            let call = ToolSet::proposal_call(name, &arguments);
            let answer = tools.call(actor, name, arguments).await;
            let action = answer.as_ref().ok().and_then(|output| {
                let ProposalCall { proposal, discards } = call?;
                let tool = name.to_owned();
                Some(if discards {
                    Action::Discarded { tool, proposal }
                } else {
                    let output: ProposalOutput = serde_json::from_value(output.clone()).ok()?;
                    Action::Proposed {
                        tool,
                        proposal,
                        destination: output.destination().clone(),
                        because: Because::Asked,
                    }
                })
            });
            Outcome { answer, action }
        }
        Decision::Read => Outcome {
            answer: tools.call(actor, name, arguments).await,
            action: None,
        },
    }
}

/// A tool call answered with `error` and not run.
fn not_run(error: ToolError) -> Outcome {
    Outcome {
        answer: Err(error),
        action: None,
    }
}

/// Drafts `drafted`, what the tool `name` would have written, as one proposal against the
/// same destination and revision, under an id derived from its patch id, so the same write
/// asked again is answered with the same proposal (H5, I6).
async fn propose(
    tools: &ToolSet,
    actor: &Actor,
    name: &str,
    drafted: &DomainPatch,
    because: Because,
) -> Outcome {
    let patch = drafted.patch();
    let proposal = proposal_id(patch);
    let mutations = patch.mutations.as_slice().to_vec();
    let answer = match draft(name, drafted.note(), patch.base_revision, mutations) {
        Ok(draft) => {
            let create = CreateProposal {
                proposal: proposal.clone(),
                destination: patch.target.domain(),
                draft,
                patch_id: patch.id.clone(),
            };
            tools.create_proposal_as(actor, create).await
        }
        Err(error) => Err(error),
    };
    match answer {
        Ok(output) => {
            patina_dst::sometimes!(
                matches!(because, Because::Structural),
                "assistant-structural-write-proposed"
            );
            let answer = json!({ "drafted_as_proposal": because, "proposal": output });
            Outcome {
                answer: Ok(answer),
                action: Some(Action::Proposed {
                    tool: name.to_owned(),
                    proposal,
                    destination: patch.target.domain(),
                    because,
                }),
            }
        }
        Err(error) => Outcome {
            answer: Err(error),
            action: None,
        },
    }
}

/// The content of a proposal the assistant drafts for the tool `tool`: `mutations` against
/// `destination_revision`, titled and described by the write's note.
///
/// # Errors
///
/// [`ToolError::Arguments`] when there are more mutations than one proposal holds, or one of
/// them edits a proposal.
pub(crate) fn draft(
    tool: &str,
    note: Option<&Markdown>,
    destination_revision: Revision,
    mutations: Vec<Mutation>,
) -> Result<ProposalDraft, ToolError> {
    let count = mutations.len();
    let mutations = BoundedVec::new(mutations).map_err(|error| ToolError::Arguments {
        path: "draft.mutations".to_owned(),
        message: error.to_string(),
    })?;
    let title = title(tool, note.map(Markdown::as_str), count);
    ProposalDraft::new(
        title,
        note.cloned(),
        destination_revision,
        mutations,
        BoundedVec::default(),
    )
    .map_err(|error| ToolError::Arguments {
        path: "draft.mutations".to_owned(),
        message: error.to_string(),
    })
}

/// The proposal id a write drafted as a proposal is filed under: `pr_` and a digest of its
/// patch id.
pub(crate) fn proposal_id(patch: &Patch) -> ProposalId {
    proposal_id_of(patch.id.as_str())
}

/// The proposal id filed under for `seed`: `pr_` and a digest of it.
pub(crate) fn proposal_id_of(seed: &str) -> ProposalId {
    let digest = Sha256::digest(seed.as_bytes());
    let hex = crate::hex(&digest[..12]);
    match format!("pr_{hex}").parse() {
        Ok(id) => id,
        Err(error) => unreachable!("`pr_` and hex digits is a proposal id: {error:?}"),
    }
}

/// A proposal's title: the first line of the write's note, or what drafted it.
pub(crate) fn title(tool: &str, note: Option<&str>, count: usize) -> Title {
    let line = note
        .and_then(|note| note.lines().find(|line| !line.trim().is_empty()))
        .map(|line| line.trim().chars().take(200).collect::<String>());
    let fallback = format!("{count} change(s) from {tool}, drafted by the assistant");
    line.and_then(|line| line.parse().ok())
        .or_else(|| fallback.parse().ok())
        .unwrap_or_else(|| match "Drafted by the assistant".parse() {
            Ok(title) => title,
            Err(error) => unreachable!("a short line is a title: {error:?}"),
        })
}

/// The applied action a direct write's output reports: the answer carries the revision and
/// what the write caused, and the patch it drafted carries its id and domain.
fn applied(
    tool: &str,
    output: &Value,
    nodes: &BTreeSet<NodeKey>,
    (patch_id, domain): (&PatchId, &Domain),
) -> Option<Action> {
    let written: WriteOutput = serde_json::from_value(output.clone()).ok()?;
    let already = written.is_already_applied();
    let (revision, consequences) = written.into_parts();
    let (tool, receipt) = (
        tool.to_owned(),
        AppliedReceipt {
            patch_id: patch_id.clone(),
            domain: domain.clone(),
            revision,
        },
    );
    Some(if already {
        Action::AlreadyApplied { tool, receipt }
    } else {
        Action::Applied {
            tool,
            receipt,
            nodes: nodes.iter().cloned().collect(),
            consequences,
        }
    })
}
