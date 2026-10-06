//! The write wrapper (ARCHITECTURE, Assistant; PRD I5, I7): every tool call the assistant
//! makes passes through it on its way to the MCP tool set, in process and as the user it
//! acts for. Reads pass through. A write's domain patch is worked out without writing
//! ([`ToolSet::drafted_patch`]) and decided by mutation kind and destination:
//!
//! - structural (PRD glossary, Structural change; every write to a route or its draft) is
//!   drafted as one proposal instead, against the same destination and revision;
//! - state touching more than [`DIRECT_WRITE_NODE_COUNT_MAX`] nodes is drafted as one
//!   proposal too, so a misheard bulk request is seen whole before it lands;
//! - any other state change applies directly, as one patch, and is reported with its
//!   consequences (D7).
//!
//! The proposal tools draft proposals already and pass through. Applying a proposal is the
//! user's click (I7), and importing a route file is the user's too, so both are refused; so
//! is any write tool whose patch the wrapper cannot see, which keeps a write tool added later
//! from slipping past it.

use std::collections::{BTreeMap, BTreeSet};

use cairn_mcp::{ToolError, ToolSet};
use cairn_schema::{
    Actor, ChangeClass, Consequences, GraphKey, JourneyId, NodeKey, Patch, PatchReceipt,
    ProposalId, RecordKey, Title,
};
use cairn_service::DomainPatch;
use cairn_store::Store;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

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
}

/// What a tool call came to: what the model is told, and the write it made, if any.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    /// The tool's answer, or why it was not run.
    pub answer: Result<Value, ToolError>,
    /// The write, when one was made.
    pub action: Option<Action>,
}

/// The policy for the tool `name` with `arguments`, with the domain patch a write drafted
/// as a proposal would have submitted.
///
/// # Errors
///
/// What working the patch out answers: arguments that do not match, or a failed read.
pub async fn decide<S: Store + 'static>(
    tools: &ToolSet<S>,
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
    Ok(match policy_of(drafted.patch()) {
        Policy::Propose(because) => Decision::Propose(because, drafted),
        _ => Decision::Direct,
    })
}

/// What [`decide`] settles for one call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Passed through.
    Read,
    /// Applied as one patch.
    Direct,
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
            Decision::Direct => Policy::Direct,
            Decision::Propose(because, _) => Policy::Propose(because.clone()),
            Decision::Proposal => Policy::Proposal,
            Decision::Refused(why) => Policy::Refused(why.clone()),
        }
    }
}

/// I5: a patch is drafted as a proposal when it is structural or touches more than the
/// direct-write node limit; otherwise it applies directly.
#[must_use]
pub fn policy_of(patch: &Patch) -> Policy {
    if patch.change_class() == ChangeClass::Structural {
        return Policy::Propose(Because::Structural);
    }
    match touched_node_count(patch) {
        Some(count) if count <= DIRECT_WRITE_NODE_COUNT_MAX => Policy::Direct,
        count => Policy::Propose(Because::TooManyNodes { count }),
    }
}

/// The nodes `patch` writes, by its touched set (H5): every record it writes on a node
/// counts that node once. `None` when it touches a whole domain or graph, so it may write
/// every node.
#[must_use]
pub fn touched_node_count(patch: &Patch) -> Option<u32> {
    let mut nodes: BTreeSet<&NodeKey> = BTreeSet::new();
    let touched = patch.touched();
    for key in touched.as_set() {
        match key {
            RecordKey::Domain(_) | RecordKey::Graph(_) | RecordKey::RouteDraft(_) => return None,
            RecordKey::InGraph { key, .. } => nodes.extend(node_of(key)),
            RecordKey::JourneyHeader(_)
            | RecordKey::RouteHeader(_)
            | RecordKey::RouteVersion { .. }
            | RecordKey::DeletedJourney(_)
            | RecordKey::Entity(_)
            | RecordKey::EntityAlias(_)
            | RecordKey::Proposal { .. } => {}
        }
    }
    Some(u32::try_from(nodes.len()).unwrap_or(u32::MAX))
}

/// The node a record inside a graph hangs off, if it hangs off one.
fn node_of(key: &GraphKey) -> Option<&NodeKey> {
    match key {
        GraphKey::Node(node)
        | GraphKey::NodeField { node, .. }
        | GraphKey::Participation { node, .. }
        | GraphKey::Resource { node, .. }
        | GraphKey::NodeState(node)
        | GraphKey::LocalEdit { node, .. }
        | GraphKey::Answer(node)
        | GraphKey::Pin(node)
        | GraphKey::Snooze(node)
        | GraphKey::Overrides(node)
        | GraphKey::Tombstone(node) => Some(node),
        GraphKey::Edge(edge) => Some(&edge.node),
        GraphKey::Annotation { node, .. } => node.as_ref(),
        GraphKey::Role(_)
        | GraphKey::Kind(_)
        | GraphKey::DefaultOwner
        | GraphKey::RetiredKey(_)
        | GraphKey::RoleFill(_) => None,
    }
}

/// Runs the tool call `name` with `arguments` for `actor` (the assistant acting for its
/// user, H2) through the policy.
pub async fn run<S: Store + 'static>(
    tools: &ToolSet<S>,
    actor: &Actor,
    name: &str,
    arguments: Value,
) -> Outcome {
    let decision = match decide(tools, name, &arguments).await {
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
        Decision::Direct => {
            let answer = tools.call(actor, name, arguments).await;
            let action = answer
                .as_ref()
                .ok()
                .and_then(|output| applied(name, output));
            patina_dst::sometimes!(action.is_some(), "assistant-direct-write-applied");
            Outcome { answer, action }
        }
        Decision::Proposal => {
            let proposal = arguments.get("proposal").cloned();
            let answer = tools.call(actor, name, arguments).await;
            let action = answer.as_ref().ok().and_then(|_| {
                let proposal = serde_json::from_value(proposal?).ok()?;
                Some(Action::Proposed {
                    tool: name.to_owned(),
                    proposal,
                    because: Because::Asked,
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
fn proposal_id(patch: &Patch) -> ProposalId {
    let digest = Sha256::digest(patch.id.as_str().as_bytes());
    let hex = crate::hex(&digest[..12]);
    match format!("pr_{hex}").parse() {
        Ok(id) => id,
        Err(error) => unreachable!("`pr_` and hex digits is a proposal id: {error:?}"),
    }
}

/// A proposal's title: the first line of the write's note, or what drafted it.
fn title(tool: &str, note: Option<&str>, count: usize) -> Title {
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
fn applied(tool: &str, output: &Value) -> Option<Action> {
    let written: Written = serde_json::from_value(output.clone()).ok()?;
    Some(Action::Applied {
        tool: tool.to_owned(),
        receipt: written.receipt,
        consequences: written.consequences,
    })
}
