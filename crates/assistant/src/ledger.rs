//! A turn's direct writes (I5;
//! decisions/2026-10-06-the-write-wrapper-decides-on-the-patch-a-tool-would-submit.md): "mark
//! these fifteen done" is one change however many tool calls a model splits it into, so
//! the ten-node limit holds across the turn. The ledger keeps the nodes the turn's direct
//! writes touched; a write that would carry them past the limit is drafted instead, and
//! every such write after it in the turn joins the same proposal for its destination, so
//! the overflow is seen whole.

use std::collections::{BTreeMap, BTreeSet};

use cairn_mcp::{ToolError, ToolSet};
use cairn_schema::{Actor, Domain, Mutation, NodeKey, ProposalId, Revision};
use cairn_service::DomainPatch;
use cairn_store::Store;
use serde_json::{Value, json};

use crate::wrapper::{Action, Because, Outcome, proposal_id_of, title};

/// The turn's direct writes and the proposals its overflow went to.
#[derive(Debug, Default)]
pub struct Ledger {
    /// The nodes the turn's applied direct writes touched, by domain.
    written: BTreeSet<(Domain, NodeKey)>,
    /// The nodes the turn's overflow drafted, by domain.
    overflowed: BTreeSet<(Domain, NodeKey)>,
    /// The overflow proposal for each destination, once there is one.
    overflow: BTreeMap<Domain, Overflow>,
}

/// One destination's overflow proposal, as the turn has drafted it so far.
#[derive(Debug)]
struct Overflow {
    proposal: ProposalId,
    mutations: Vec<Mutation>,
    destination_revision: Revision,
    /// Its editing revision, which the next edit names.
    editing: Revision,
    /// The edits made to it, for each edit's patch id.
    edits: u32,
}

impl Ledger {
    /// How many nodes the turn's direct writes would have touched with `nodes` in `domain`.
    #[must_use]
    pub fn with(&self, domain: &Domain, nodes: &BTreeSet<NodeKey>) -> usize {
        let fresh = nodes
            .iter()
            .filter(|node| !self.written.contains(&(domain.clone(), (*node).clone())))
            .count();
        self.written.len() + fresh
    }

    /// How many nodes the turn's writes, landed and drafted, would have touched with `nodes`
    /// in `domain`.
    fn asked(&self, domain: &Domain, nodes: &BTreeSet<NodeKey>) -> usize {
        let mut asked: BTreeSet<&(Domain, NodeKey)> =
            self.written.iter().chain(&self.overflowed).collect();
        let fresh: Vec<(Domain, NodeKey)> = nodes
            .iter()
            .map(|node| (domain.clone(), node.clone()))
            .collect();
        asked.extend(&fresh);
        asked.len()
    }

    /// Records a direct write that landed.
    pub fn wrote(&mut self, domain: &Domain, nodes: BTreeSet<NodeKey>) {
        self.written
            .extend(nodes.into_iter().map(|node| (domain.clone(), node)));
    }

    /// Drafts `drafted`, a direct write touching `nodes` that would carry the turn past the
    /// limit, into its destination's overflow proposal: created against the destination's
    /// `current` revision on the first, replaced with every overflowing mutation so far on
    /// each after it.
    pub(crate) async fn overflow<S: Store + 'static>(
        &mut self,
        tools: &ToolSet<S>,
        actor: &Actor,
        (tool, note): (&str, Option<&str>),
        drafted: &DomainPatch,
        current: Option<Revision>,
        nodes: &BTreeSet<NodeKey>,
    ) -> Outcome {
        let patch = drafted.patch();
        let domain = patch.target.domain();
        let count = u32::try_from(self.asked(&domain, nodes)).unwrap_or(u32::MAX);
        let because = Because::TooManyNodesThisTurn { count };
        let mutations = patch.mutations.as_slice();
        let answer = if let Some(held) = self.overflow.get_mut(&domain) {
            held.mutations.extend(mutations.iter().cloned());
            let edited = held.edit(tools, actor, tool, note).await;
            if edited.is_err() {
                let kept = held.mutations.len() - mutations.len();
                held.mutations.truncate(kept);
            }
            edited.map(|output| (held.proposal.clone(), output))
        } else {
            let mut held = Overflow {
                proposal: proposal_id_of(&format!("overflow {}", patch.id)),
                mutations: mutations.to_vec(),
                destination_revision: current.unwrap_or(patch.base_revision),
                editing: Revision::NONE,
                edits: 0,
            };
            let created = held
                .create(tools, actor, &domain, (tool, note), patch)
                .await;
            created.map(|output| {
                let proposal = held.proposal.clone();
                self.overflow.insert(domain.clone(), held);
                (proposal, output)
            })
        };
        if answer.is_ok() {
            let drafted = nodes.iter().map(|node| (domain.clone(), node.clone()));
            self.overflowed.extend(drafted);
        }
        patina_dst::sometimes!(answer.is_ok(), "assistant-turn-overflow-proposed");
        match answer {
            Ok((proposal, output)) => Outcome {
                answer: Ok(json!({ "drafted_as_proposal": because, "proposal": output })),
                action: Some(Action::Proposed {
                    tool: tool.to_owned(),
                    proposal,
                    because,
                }),
            },
            Err(error) => Outcome {
                answer: Err(error),
                action: None,
            },
        }
    }
}

impl Overflow {
    /// Its draft as the proposal tools take it.
    fn draft(&self, tool: &str, note: Option<&str>) -> Value {
        json!({
            "title": title(tool, note, self.mutations.len()),
            "description": note,
            "destination_revision": self.destination_revision,
            "mutations": self.mutations,
        })
    }

    /// Creates it, under the overflowing patch's id.
    async fn create<S: Store + 'static>(
        &mut self,
        tools: &ToolSet<S>,
        actor: &Actor,
        destination: &Domain,
        (tool, note): (&str, Option<&str>),
        patch: &cairn_schema::Patch,
    ) -> Result<Value, ToolError> {
        let create = json!({
            "proposal": self.proposal,
            "destination": destination,
            "draft": self.draft(tool, note),
            "patch_id": patch.id,
        });
        let output = tools.call(actor, "create_proposal", create).await?;
        self.editing = editing_revision(&output);
        Ok(output)
    }

    /// Replaces its content with every overflowing mutation so far.
    async fn edit<S: Store + 'static>(
        &mut self,
        tools: &ToolSet<S>,
        actor: &Actor,
        tool: &str,
        note: Option<&str>,
    ) -> Result<Value, ToolError> {
        self.edits += 1;
        let patch_id = format!("p_{}_edit_{}", self.proposal.as_str(), self.edits);
        let edit = json!({
            "proposal": self.proposal,
            "change": { "replace": { "draft": self.draft(tool, note) } },
            "base_revision": self.editing,
            "patch_id": patch_id,
        });
        let output = tools.call(actor, "edit_proposal", edit).await?;
        self.editing = editing_revision(&output);
        Ok(output)
    }
}

/// The editing revision a proposal tool's output names: the proposal's own, or its
/// receipt's.
fn editing_revision(output: &Value) -> Revision {
    let named = output["proposal"]["revision"]
        .as_u64()
        .or_else(|| output["receipt"]["revision"].as_u64());
    let revision = named.and_then(|revision| u32::try_from(revision).ok());
    serde_json::from_value(json!(revision.unwrap_or(0))).unwrap_or(Revision::NONE)
}
