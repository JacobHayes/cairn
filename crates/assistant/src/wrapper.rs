//! The write wrapper (ARCHITECTURE, Assistant; PRD I5, I7): every tool call the assistant
//! makes passes through it on its way to the MCP tool set, in process and as the user it
//! acts for. Reads pass through. A write's domain patch is worked out without writing
//! ([`ToolSet::drafted_patch`]) and decided by mutation kind and destination:
//!
//! - structural (PRD glossary, Structural change; every write to a route or its draft) is
//!   drafted as one proposal instead, against the same destination and revision;
//! - state touching more than [`DIRECT_WRITE_NODE_COUNT_MAX`] nodes ([`touched_nodes`]) is
//!   drafted as one proposal too, so a misheard bulk request is seen whole before it lands;
//! - any other state change applies directly, as one patch, and is reported with its
//!   consequences (D7), as long as the turn's direct writes together stay within the same
//!   limit: a bulk request split into many calls is still one change, and what would carry
//!   the turn past the limit is drafted as one proposal ([`Ledger`]).
//!
//! The proposal tools draft proposals already and pass through. Applying a proposal is the
//! user's click (I7), and importing a route file is the user's too, so both are refused; so
//! is any write tool whose patch the wrapper cannot see, which keeps a write tool added later
//! from slipping past it.

use std::collections::{BTreeMap, BTreeSet};

use cairn_mcp::{ToolError, ToolSet};
use cairn_schema::{
    Actor, ChangeClass, Consequences, Graph, JourneyId, NodeKey, Patch, PatchReceipt, PatchTarget,
    ProposalId, Revision, Title,
};
use cairn_service::DomainPatch;
use cairn_store::{Document, LoadTarget, Store};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub use crate::ledger::Ledger;
pub use crate::touched::touched_nodes;

use crate::limits::DIRECT_WRITE_NODE_COUNT_MAX;

/// The tools that draft a proposal themselves: they pass through, since review is where a
/// proposal goes.
pub const PROPOSAL_TOOLS: [&str; 5] = [
    "create_proposal",
    "edit_proposal",
    "upgrade",
    "save_as_route",
    "relink",
];

/// The write tools the assistant never runs, with why: what they do is the user's to do.
pub const REFUSED_TOOLS: [(&str, &str); 2] = [
    (
        "apply_proposal",
        "applying a proposal is the user's click: tell them it is ready for review",
    ),
    (
        "import_route",
        "importing a route file is the user's to do; draft the route's nodes as a proposal \
         instead",
    ),
];

/// What the wrapper does with one tool call (I5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Policy {
    /// A read, or a name no tool has: passed through.
    Read,
    /// A state change within the node limit: applied as one patch.
    Direct,
    /// A write drafted as one proposal instead, and why.
    Propose(Because),
    /// A proposal tool: passed through.
    Proposal,
    /// Not run, and why.
    Refused(String),
}

/// Why a write became a proposal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum Because {
    /// It changes structure, or writes a route or its draft (PRD glossary, Structural change).
    Structural,
    /// It touches more nodes than one direct write may (I5).
    TooManyNodes {
        /// The nodes it writes; absent when it may write every node of its domain.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        count: Option<u32>,
    },
    /// It would carry the nodes the turn's direct writes touch past the limit one change
    /// may (I5): with every later such write in the turn, one proposal.
    TooManyNodesThisTurn {
        /// The nodes the turn's direct writes and this one would touch together.
        count: u32,
    },
    /// The assistant drafted it as a proposal itself.
    Asked,
}

/// A write the assistant made, as the turn reports it (I5: every direct write is reported
/// with its consequences).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Action {
    /// Applied directly, as one patch.
    Applied {
        /// The tool.
        tool: String,
        /// The patch's receipt.
        receipt: PatchReceipt,
        /// The nodes it wrote, in key order (I5: a direct change is reported with the nodes
        /// it affected, which the panel links to); none for a write to no journey's nodes.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        nodes: Vec<NodeKey>,
        /// What it newly caused in each journey it changed (D7).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        consequences: BTreeMap<JourneyId, Consequences>,
    },
    /// Drafted as a proposal for the user to review and apply.
    Proposed {
        /// The tool.
        tool: String,
        /// The proposal.
        proposal: ProposalId,
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
            Action::Applied { .. } => None,
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

/// The policy for the tool `name` with `arguments`, with the domain patch a write would
/// submit, and for a direct write the nodes it touches, read against its journey in `store`.
///
/// # Errors
///
/// What working the patch out answers: arguments that do not match, or a failed read.
pub async fn decide<S: Store + 'static>(
    tools: &ToolSet<S>,
    store: &S,
    name: &str,
    arguments: &Value,
) -> Result<Decision, ToolError> {
    let writes = ToolSet::<S>::definitions()
        .into_iter()
        .find(|definition| definition.name == name)
        .is_some_and(|definition| definition.writes);
    if !writes {
        return Ok(Decision::Read);
    }
    if let Some((_, why)) = REFUSED_TOOLS.iter().find(|(refused, _)| *refused == name) {
        return Ok(Decision::Refused((*why).to_owned()));
    }
    let Some(drafted) = tools.drafted_patch(name, arguments.clone()).await? else {
        if PROPOSAL_TOOLS.contains(&name) {
            return Ok(Decision::Proposal);
        }
        return Ok(Decision::Refused(format!(
            "the assistant cannot tell what {name} would change, so it does not run it"
        )));
    };
    if drafted.patch().change_class() == ChangeClass::Structural {
        return Ok(Decision::Propose(Because::Structural, drafted));
    }
    let stored = stored_journey(store, drafted.patch()).await?;
    let graph = stored.as_ref().map(|(graph, _)| graph);
    let current = stored.as_ref().map(|(_, revision)| *revision);
    Ok(match touched_nodes(drafted.patch(), graph) {
        Some(nodes) if within_limit(nodes.len()) => Decision::Direct {
            drafted,
            nodes,
            current,
        },
        nodes => {
            let count = nodes.map(|nodes| u32::try_from(nodes.len()).unwrap_or(u32::MAX));
            Decision::Propose(Because::TooManyNodes { count }, drafted)
        }
    })
}

/// Whether `count` nodes are within what one direct change may touch.
pub(crate) fn within_limit(count: usize) -> bool {
    u32::try_from(count).is_ok_and(|count| count <= DIRECT_WRITE_NODE_COUNT_MAX)
}

/// The journey `patch` targets, as stored, with its revision; `None` for another domain or
/// a journey that does not exist yet.
async fn stored_journey<S: Store>(
    store: &S,
    patch: &Patch,
) -> Result<Option<(Graph, Revision)>, ToolError> {
    let PatchTarget::Journey(journey) = &patch.target else {
        return Ok(None);
    };
    let loaded = store.load(&LoadTarget::Journey(journey.clone())).await;
    let loaded = loaded.map_err(|error| ToolError::Failed {
        message: error.to_string(),
    })?;
    Ok(match loaded {
        Some(Document::Journey(journey)) => Some((journey.graph, journey.revision)),
        _ => None,
    })
}

/// What [`decide`] settles for one call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Passed through.
    Read,
    /// Applied as one patch, if the turn's direct writes stay within the limit with it.
    Direct {
        /// The patch.
        drafted: DomainPatch,
        /// The nodes it touches.
        nodes: BTreeSet<NodeKey>,
        /// Its journey's current revision, when it targets one that exists.
        current: Option<Revision>,
    },
    /// Drafted as one proposal of this patch's mutations instead.
    Propose(Because, DomainPatch),
    /// A proposal tool: passed through.
    Proposal,
    /// Not run.
    Refused(String),
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
        }
    }
}

/// Runs the tool call `name` with `arguments` for `actor` (the assistant acting for its
/// user, H2) through the policy, keeping the turn's direct writes in `ledger`.
pub async fn run<S: Store + 'static>(
    tools: &ToolSet<S>,
    store: &S,
    actor: &Actor,
    (name, arguments): (&str, Value),
    ledger: &mut Ledger,
) -> Outcome {
    let decision = match decide(tools, store, name, &arguments).await {
        Ok(decision) => decision,
        Err(error) => {
            return Outcome {
                answer: Err(error),
                action: None,
            };
        }
    };
    match decision {
        Decision::Refused(why) => Outcome {
            answer: Err(ToolError::Refused { message: why }),
            action: None,
        },
        Decision::Propose(because, drafted) => {
            propose(tools, actor, name, &arguments, &drafted, because).await
        }
        Decision::Direct {
            drafted,
            nodes,
            current,
        } => {
            let domain = drafted.patch().target.domain();
            let count = ledger.with(&domain, &nodes);
            if !within_limit(count) {
                let note = arguments.get("note").and_then(Value::as_str);
                return ledger
                    .overflow(tools, actor, (name, note), &drafted, current, &nodes)
                    .await;
            }
            let answer = tools.call(actor, name, arguments).await;
            let action = answer
                .as_ref()
                .ok()
                .and_then(|output| applied(name, output, &nodes));
            if action.is_some() {
                ledger.wrote(&domain, nodes);
            }
            patina_dst::sometimes!(action.is_some(), "assistant-direct-write-applied");
            Outcome { answer, action }
        }
        Decision::Proposal => {
            let proposal = arguments.get("proposal").cloned();
            let discards = arguments.get("change").and_then(Value::as_str) == Some("discard");
            let answer = tools.call(actor, name, arguments).await;
            let action = answer.as_ref().ok().and_then(|_| {
                let proposal = serde_json::from_value(proposal?).ok()?;
                let tool = name.to_owned();
                Some(if discards {
                    Action::Discarded { tool, proposal }
                } else {
                    Action::Proposed {
                        tool,
                        proposal,
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

/// Drafts `drafted`, what the tool `name` would have written, as one proposal against the
/// same destination and revision, under an id derived from its patch id, so the same write
/// asked again is answered with the same proposal (H5, I6).
async fn propose<S: Store + 'static>(
    tools: &ToolSet<S>,
    actor: &Actor,
    name: &str,
    arguments: &Value,
    drafted: &DomainPatch,
    because: Because,
) -> Outcome {
    let patch = drafted.patch();
    let proposal = proposal_id(patch);
    let note = arguments.get("note").and_then(Value::as_str);
    let mutations = patch.mutations.as_slice();
    let draft = json!({
        "title": title(name, note, mutations.len()),
        "description": note,
        "destination_revision": patch.base_revision,
        "mutations": mutations,
    });
    let create = json!({
        "proposal": proposal,
        "destination": patch.target.domain(),
        "draft": draft,
        "patch_id": patch.id,
    });
    let answer = tools.call(actor, "create_proposal", create).await;
    match answer {
        Ok(output) => {
            patina_dst::sometimes!(
                matches!(because, Because::TooManyNodes { .. }),
                "assistant-bulk-write-proposed"
            );
            let answer = json!({ "drafted_as_proposal": because, "proposal": output });
            Outcome {
                answer: Ok(answer),
                action: Some(Action::Proposed {
                    tool: name.to_owned(),
                    proposal,
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

/// A direct write's output, as the tool set answers it.
#[derive(Deserialize)]
struct Written {
    receipt: PatchReceipt,
    #[serde(default)]
    consequences: BTreeMap<JourneyId, Consequences>,
}

/// The applied action a direct write's output reports.
fn applied(tool: &str, output: &Value, nodes: &BTreeSet<NodeKey>) -> Option<Action> {
    let written: Written = serde_json::from_value(output.clone()).ok()?;
    Some(Action::Applied {
        tool: tool.to_owned(),
        receipt: written.receipt,
        nodes: nodes.iter().cloned().collect(),
        consequences: written.consequences,
    })
}
