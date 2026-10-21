//! A turn's direct writes (I5;
//! decisions/2026-10-06-the-write-wrapper-decides-on-the-patch-a-tool-would-submit.md): "mark
//! these fifteen done" is one change however many tool calls a model splits it into, so
//! the ten-node limit holds across the turn. The ledger keeps the nodes the turn's direct
//! writes touched; a write that would carry them past the limit is drafted instead, and
//! every such write after it in the turn joins the same proposal for its destination, so
//! the overflow is seen whole.

use std::collections::{BTreeMap, BTreeSet};

use cairn_mcp::{CreateProposal, EditProposal, ProposalChange, ProposalOutput, ToolError, ToolSet};
use cairn_schema::{Actor, Domain, Markdown, Mutation, NodeKey, Patch, ProposalId, Revision};
use cairn_service::DomainPatch;
use serde_json::json;

use crate::wrapper::{Action, Because, Outcome, draft, proposal_id_of};

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
    pub(crate) async fn overflow(
        &mut self,
        tools: &ToolSet,
        actor: &Actor,
        tool: &str,
        drafted: &DomainPatch,
        current: Option<Revision>,
        nodes: &BTreeSet<NodeKey>,
    ) -> Outcome {
        let patch = drafted.patch();
        let note = drafted.note();
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
                    destination: domain.clone(),
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
    /// Creates it, under the overflowing patch's id.
    async fn create(
        &mut self,
        tools: &ToolSet,
        actor: &Actor,
        destination: &Domain,
        (tool, note): (&str, Option<&Markdown>),
        patch: &Patch,
    ) -> Result<ProposalOutput, ToolError> {
        let create = CreateProposal {
            proposal: self.proposal.clone(),
            destination: destination.clone(),
            draft: draft(
                tool,
                note,
                self.destination_revision,
                self.mutations.clone(),
            )?,
            patch_id: patch.id.clone(),
        };
        let output = tools.create_proposal_as(actor, create).await?;
        self.editing = output.editing_revision().unwrap_or(Revision::NONE);
        Ok(output)
    }

    /// Replaces its content with every overflowing mutation so far.
    async fn edit(
        &mut self,
        tools: &ToolSet,
        actor: &Actor,
        tool: &str,
        note: Option<&Markdown>,
    ) -> Result<ProposalOutput, ToolError> {
        self.edits += 1;
        let patch_id = format!("p_{}_edit_{}", self.proposal.as_str(), self.edits);
        let patch_id = patch_id.parse().map_err(|error| ToolError::Failed {
            message: format!("an overflow edit's patch id did not parse: {error:?}"),
        })?;
        let draft = draft(
            tool,
            note,
            self.destination_revision,
            self.mutations.clone(),
        )?;
        let edit = EditProposal {
            proposal: self.proposal.clone(),
            change: ProposalChange::Replace { draft },
            base_revision: self.editing,
            patch_id,
        };
        let output = tools.edit_proposal_as(actor, edit).await?;
        self.editing = output.editing_revision().unwrap_or(Revision::NONE);
        Ok(output)
    }
}
