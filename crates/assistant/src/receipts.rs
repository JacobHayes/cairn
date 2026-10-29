//! What a turn reports of its writes (I5): read from the tools' own answers, never worked out
//! beforehand. A write to a journey, route or the deployment is a [`Write`] with the revision
//! it produced and what it newly caused (D7); a proposal drafted, changed or discarded is a
//! [`Proposed`]. Each is also a plain line the conversation keeps.

use std::collections::BTreeMap;

use cairn_mcp::{ProposalOutput, WriteOutput, Writes};
use cairn_schema::{Consequences, Domain, JourneyId, PatchId, ProposalId, Revision};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::conversation::Target;

/// A direct write the assistant made.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Write {
    /// The tool.
    pub tool: String,
    /// The patch id the write was sent under.
    pub patch_id: PatchId,
    /// What it wrote to.
    pub domain: Domain,
    /// The revision it produced, or that the patch id was already applied at.
    pub revision: Revision,
    /// The patch id was applied before with the same content, and nothing was written again.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub already_applied: bool,
    // PRD: D7.
    /// What it newly caused in each journey it changed.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub consequences: BTreeMap<JourneyId, Consequences>,
}

/// A proposal the assistant drafted, changed or discarded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Proposed {
    /// The proposal.
    pub proposal: ProposalId,
    // PRD: I6.
    /// Where it would land, under which it is addressed.
    pub destination: Domain,
    /// The assistant discarded it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub discarded: bool,
}

/// The writes of one turn, in the order they were made.
#[derive(Debug, Default)]
pub(crate) struct Receipts {
    writes: Vec<Write>,
    proposals: Vec<Proposed>,
}

impl Receipts {
    /// Notes the answer `output` of a successful call of `tool` with `arguments`, which
    /// writes `writes`.
    pub(crate) fn record(&mut self, writes: Writes, tool: &str, arguments: &Value, output: &Value) {
        match writes {
            Writes::Patch => self.write(tool, arguments, output),
            Writes::Proposal => self.proposal(arguments, output),
            Writes::Nothing | Writes::File | Writes::Confirmation => {}
        }
    }

    fn write(&mut self, tool: &str, arguments: &Value, output: &Value) {
        let patch_id = id_in(arguments, "patch_id");
        let written = serde_json::from_value::<WriteOutput>(output.clone());
        let (Some(patch_id), Ok(written)) = (patch_id, written) else {
            tracing::warn!(tool, "an assistant write could not be reported");
            return;
        };
        let already_applied = written.is_already_applied();
        let (revision, consequences) = written.into_parts();
        self.writes.push(Write {
            tool: tool.to_owned(),
            patch_id,
            domain: domain_of(arguments),
            revision,
            already_applied,
            consequences,
        });
    }

    /// A later write to a proposal the turn already reported replaces that report, so each
    /// proposal is reported once, as it stands.
    fn proposal(&mut self, arguments: &Value, output: &Value) {
        let proposal = id_in(arguments, "proposal");
        let output = serde_json::from_value::<ProposalOutput>(output.clone());
        let (Some(proposal), Ok(output)) = (proposal, output) else {
            tracing::warn!("an assistant proposal could not be reported");
            return;
        };
        // An answer from a receipt (`already_saved`) says nothing of where the proposal
        // stands: an earlier report of it in the turn stands, and with none the call's own
        // change is what is known.
        let known = self
            .proposals
            .iter()
            .position(|each| each.proposal == proposal);
        let discarded = if output.state_known() {
            output.is_discarded()
        } else if known.is_some() {
            return;
        } else {
            arguments["change"] == "discard"
        };
        let held = Proposed {
            proposal,
            destination: output.destination().clone(),
            discarded,
        };
        match known {
            Some(at) => self.proposals[at] = held,
            None => self.proposals.push(held),
        }
    }

    /// Whether the turn wrote anything.
    pub(crate) fn is_empty(&self) -> bool {
        self.writes.is_empty() && self.proposals.is_empty()
    }

    /// How many writes and proposals the turn made.
    pub(crate) fn len(&self) -> usize {
        self.writes.len() + self.proposals.len()
    }

    /// One plain line per write and proposal, which the conversation keeps: in words, naming
    /// no id, since the person reads them (the answer to the turn carries the ids).
    pub(crate) fn lines(&self, target: &Target) -> Vec<String> {
        let writes = self.writes.iter().map(|write| {
            let place = place(&write.domain, target);
            if write.already_applied {
                format!("That change was already applied to {place}.")
            } else {
                format!("Applied a change to {place}.")
            }
        });
        let proposals = self.proposals.iter().map(|held| {
            let place = place(&held.destination, target);
            if held.discarded {
                format!("Discarded a proposal for {place}.")
            } else {
                format!("A proposal for {place} is ready for review.")
            }
        });
        writes.chain(proposals).collect()
    }

    pub(crate) fn into_parts(self) -> (Vec<Write>, Vec<Proposed>) {
        (self.writes, self.proposals)
    }
}

/// The id a call's arguments name under `field`.
fn id_in<T: serde::de::DeserializeOwned>(arguments: &Value, field: &str) -> Option<T> {
    serde_json::from_value(arguments.get(field)?.clone()).ok()
}

/// What a write call's arguments aim at: its `target` (`apply_patch`), its `journey`
/// (`update_journey_state`), otherwise the deployment (`manage_identity`).
fn domain_of(arguments: &Value) -> Domain {
    id_in(arguments, "target")
        .or_else(|| id_in(arguments, "journey").map(Domain::Journey))
        .unwrap_or(Domain::Deployment)
}

/// Where a write landed, in words: "this journey" when it is the conversation's own.
fn place(domain: &Domain, target: &Target) -> &'static str {
    match (domain, target) {
        (Domain::Journey(journey), Target::Journey(of)) if journey == of => "this journey",
        (Domain::Route(route), Target::RouteDraft(of)) if route == of => "this route",
        (Domain::Journey(_), _) => "a journey",
        (Domain::Route(_), _) => "a route",
        (Domain::Deployment, _) => "the deployment's people and roles",
    }
}
