//! Proposals (I6, C14; ARCHITECTURE, Model: Proposal): created against any domain with a
//! client-generated id, edited against their own editing revision without touching their
//! destination (H5), fetched by id, previewed with their consequences (D7), applied with the
//! confirming user (H2) after both revisions are checked, and discarded. Each write is a patch
//! with a client patch id, through the same receipt-first write path as a domain patch.
//!
//! Cost: a proposal write loads the deployment and the proposal; an apply also loads the
//! destination and what the proposal's mutations read; a preview does the same loads, one
//! apply, and for a journey two derives (for a deployment proposal, one apply more and two
//! derives per journey a merge in it was checked against).

use std::collections::{BTreeMap, BTreeSet};

use cairn_engine::{ApplyInputs, Records, apply, resolve_partial};
use cairn_schema::{
    Consequences, Domain, EventType, JourneyId, Markdown, Mutation, Mutations, Patch, PatchId,
    PatchReceipt, PatchTarget, Proposal, ProposalDraft, ProposalId, ProposalPreview,
    ProposalStatus, Record, Rejection, Revision, RevisionConflict, RevisionOf, Subject, TouchedSet,
    Write,
};
use cairn_store::{Document, EventQuery, LoadTarget, PageSize, Store};

use crate::write::engine;
use crate::{Call, ReadError, Service, ServiceError, WriteError, Written, consequence, load};

/// What a write to a proposal answers (I6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposalWritten {
    /// Committed now: the receipt (its revision is the proposal's editing revision) and the
    /// proposal as it now stands.
    Saved {
        /// The receipt.
        receipt: PatchReceipt,
        /// The proposal.
        proposal: Proposal,
    },
    /// The patch id was committed before with the same content (H5): answered from its
    /// receipt; fetch the proposal by id for where it stands now.
    AlreadySaved {
        /// The original receipt.
        receipt: PatchReceipt,
    },
    /// I6: a create naming a proposal id its caller already created for this destination,
    /// as an agent that lost the response resubmits: the proposal as it stands, not a second
    /// one.
    Existing {
        /// The proposal.
        proposal: Proposal,
    },
}

/// C14, D7, I6: a proposal under review: what applying it now would do, and, when its
/// destination moved since it was drafted, what moved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalReview {
    /// The proposal, at the editing revision a reviewer applies.
    pub proposal: Proposal,
    /// Its items still unresolved, the violations of its candidate, and for a journey the
    /// graph and frontier after.
    pub preview: ProposalPreview,
    /// D7, by journey: what applying it would newly cause in each journey it changes, or
    /// whose meaning an entity merge in it changes; a journey with nothing new is left out.
    pub consequences: BTreeMap<JourneyId, Consequences>,
    /// I6: the destination moved since the proposal was drafted, so applying it is stale until
    /// it is refreshed and reviewed again.
    pub stale: Option<StaleBase>,
}

/// I6: a proposal's destination moved past the revision it was drafted against.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaleBase {
    /// The destination's revision the proposal names, and its current one.
    pub conflict: RevisionConflict,
    /// What the intervening events touched (H5).
    pub intervening: TouchedSet,
}

impl<S: Store> Service<S> {
    /// I6: creates proposal `id` for `destination` (which may not exist yet: a proposal can
    /// create a journey or route at revision 0) as `call`'s actor, who is its author; an agent
    /// is recorded as the proposing agent. Drafting never moves the destination's revision
    /// (H5). A resubmission is answered from its receipt; a create naming an id the same
    /// author already holds for this destination answers that proposal.
    ///
    /// # Errors
    ///
    /// [`WriteError::Rejected`] when the id is taken otherwise, or the patch id is reused;
    /// [`WriteError::Failed`] when the store fails.
    pub async fn create_proposal(
        &self,
        call: &Call,
        patch_id: PatchId,
        destination: &Domain,
        id: &ProposalId,
        draft: ProposalDraft,
    ) -> Result<ProposalWritten, WriteError> {
        let patch = proposal_patch(
            patch_id,
            destination,
            id,
            Revision::NONE,
            Mutation::CreateProposal { proposal: draft },
        );
        if let Some(answer) = self.saved_before(&patch).await? {
            return answer;
        }
        if let Some(held) = self.store.proposal(id).await?
            && held.destination == *destination
            && held.created_by == call.actor.user
            && held.proposing_agent == call.actor.agent
        {
            patina_dst::reachable!("service-proposal-create-resubmitted");
            return Ok(ProposalWritten::Existing { proposal: held });
        }
        self.write_proposal(call, &patch, id).await
    }

    /// I6, H5: replaces the proposal's content, against `base_revision`, its editing revision
    /// the editor saw. Its destination's revision does not move.
    ///
    /// # Errors
    ///
    /// [`WriteError::Rejected`] when the proposal moved past `base_revision` (stale), does not
    /// exist, belongs to another destination, or is no longer open; [`WriteError::Failed`]
    /// when the store fails.
    pub async fn edit_proposal(
        &self,
        call: &Call,
        patch_id: PatchId,
        destination: &Domain,
        id: &ProposalId,
        base_revision: Revision,
        draft: ProposalDraft,
    ) -> Result<ProposalWritten, WriteError> {
        let patch = proposal_patch(
            patch_id,
            destination,
            id,
            base_revision,
            Mutation::EditProposal { proposal: draft },
        );
        if let Some(answer) = self.saved_before(&patch).await? {
            return answer;
        }
        self.write_proposal(call, &patch, id).await
    }

    /// I6: discards the proposal, against its editing revision.
    ///
    /// # Errors
    ///
    /// As [`Service::edit_proposal`].
    pub async fn discard_proposal(
        &self,
        call: &Call,
        patch_id: PatchId,
        destination: &Domain,
        id: &ProposalId,
        base_revision: Revision,
    ) -> Result<ProposalWritten, WriteError> {
        let patch = proposal_patch(
            patch_id,
            destination,
            id,
            base_revision,
            Mutation::DiscardProposal {},
        );
        if let Some(answer) = self.saved_before(&patch).await? {
            return answer;
        }
        self.write_proposal(call, &patch, id).await
    }

    /// I6: the proposal with this id, with its destination, editing revision, and status, or
    /// none.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn proposal(&self, id: &ProposalId) -> Result<Option<Proposal>, ServiceError> {
        Ok(self.store.proposal(id).await?)
    }

    /// C14, D7, I6: what applying the proposal now would do, at `call`'s today: its items still
    /// unresolved (their effect left out), every violation of the candidate, the destination's
    /// graph after, a journey's frontier after, the consequences by journey, and, when the
    /// destination moved since drafting, what moved.
    ///
    /// # Errors
    ///
    /// [`ReadError::ProposalMissing`] when there is no such proposal; [`ReadError::Failed`]
    /// when the store fails or the engine panics.
    pub async fn preview_proposal(
        &self,
        call: &Call,
        id: &ProposalId,
    ) -> Result<ProposalReview, ReadError> {
        let proposal = self
            .store
            .proposal(id)
            .await?
            .ok_or_else(|| ReadError::ProposalMissing(id.clone()))?;
        let destination = proposal.destination.clone();
        // What its apply would load: the destination, the proposal, and what its mutations read.
        let reads = Patch {
            id: proposal_read_id(),
            target: domain_target(&destination),
            base_revision: proposal.draft.destination_revision,
            deployment_revision: None,
            mutations: one(Mutation::ApplyProposal {
                proposal: id.clone(),
                reviewed_revision: proposal.revision,
            }),
        };
        let loaded = load::records(&*self.store, &reads).await?;
        let records = &loaded.records;
        let stale = self
            .stale_base(
                &destination,
                proposal.draft.destination_revision,
                records.revision(&destination),
            )
            .await?;
        let today = self.settings.today(call.now);
        let inputs = ApplyInputs {
            today,
            at: call.now,
            actor: call.actor.clone(),
            note: None,
        };
        let derive_inputs =
            self.settings
                .derive_inputs(today, BTreeSet::new(), records.deployment.clone());
        let (preview, consequences) = engine(&destination, || {
            let preview = cairn_engine::preview(
                records,
                &destination,
                &proposal.draft,
                &inputs,
                &derive_inputs,
            );
            let consequences = match &destination {
                Domain::Journey(journey) => preview
                    .consequences
                    .iter()
                    .filter(|caused| **caused != Consequences::default())
                    .map(|caused| (journey.clone(), caused.clone()))
                    .collect(),
                Domain::Deployment if preview.violations.is_empty() => {
                    merged(records, &proposal.draft, &inputs, &self.settings)
                }
                Domain::Deployment | Domain::Route(_) => BTreeMap::new(),
            };
            (preview, consequences)
        })?;
        Ok(ProposalReview {
            proposal,
            preview,
            consequences,
            stale,
        })
    }

    /// I6, H2, H5: applies the proposal to `destination` as `call`'s actor, the confirming user.
    /// Its events are attributed to its author and proposing agent with the caller recorded as
    /// confirming it (H2); an agent confirms for its user, through the same checks as anyone
    /// (I7). The apply checks the editing revision the reviewer saw and the destination's
    /// revision the proposal was drafted against (both at commit too), blocks on an item still
    /// needing a choice (C14), validates the resolved mutations strictly, and commits them,
    /// their events, and the applied status in one transaction.
    ///
    /// # Errors
    ///
    /// [`WriteError::Rejected`] as stale (with what intervened) when either revision moved, as
    /// invalid with every violation, or for a reused patch id; [`WriteError::Failed`] when the
    /// store fails or the engine panics.
    pub async fn apply_proposal(
        &self,
        call: &Call,
        patch_id: PatchId,
        destination: &Domain,
        id: &ProposalId,
        reviewed_revision: Revision,
        note: Option<Markdown>,
    ) -> Result<Written, WriteError> {
        // H5: a resubmission is rebuilt as its receipt says it was committed (the destination
        // revision it applied to), since the proposal it applied is no longer open.
        let committed = self.store.receipt(&patch_id).await?;
        let base_revision = match committed {
            Some(receipt) => crate::write::before(receipt.revision),
            None => match self.store.proposal(id).await? {
                Some(found)
                    if found.destination == *destination
                        && found.status == ProposalStatus::Open =>
                {
                    found.draft.destination_revision
                }
                // At the destination's current revision, so the engine names what is wrong
                // rather than a stale base: no such proposal, another destination's, or one
                // applied or discarded.
                _ => self.current_revision(destination).await?,
            },
        };
        let patch = Patch {
            id: patch_id,
            target: domain_target(destination),
            base_revision,
            deployment_revision: None,
            mutations: one(Mutation::ApplyProposal {
                proposal: id.clone(),
                reviewed_revision,
            }),
        };
        if let Some(answer) = self.answer_from_receipt(&patch).await? {
            patina_dst::reachable!("service-proposal-apply-answered-from-receipt");
            return answer;
        }
        self.write(call, &patch, note).await
    }

    /// I6, H5: what moved in `destination` since `drafted`, when it is not `current`.
    async fn stale_base(
        &self,
        destination: &Domain,
        drafted: Revision,
        current: Revision,
    ) -> Result<Option<StaleBase>, ServiceError> {
        if current == drafted {
            return Ok(None);
        }
        let conflict = RevisionConflict {
            of: RevisionOf::Domain(destination.clone()),
            expected: drafted,
            current,
        };
        let intervening = self
            .store
            .intervening(std::slice::from_ref(&conflict))
            .await?;
        Ok(Some(StaleBase {
            conflict,
            intervening,
        }))
    }

    /// H5, I6: the answer for a proposal write whose content is drafted from its destination
    /// as it stands (a proposed upgrade, save, or re-link, or a refresh), which a resubmission
    /// after the destination moved could not rebuild byte for byte. A committed patch id is
    /// answered from its receipt when it was the same request: a proposal create (or, given
    /// `edited_from`, an edit from that revision) of this proposal for this destination, by
    /// the caller for a create, whose proposal as written `fits` the request; any other
    /// committed write under the id is refused as reused. A create naming a proposal its
    /// caller already holds for this destination, fitting the request, answers that proposal.
    pub(crate) async fn drafted_before(
        &self,
        call: &Call,
        patch_id: &PatchId,
        (destination, id): (&Domain, &ProposalId),
        edited_from: Option<Revision>,
        fits: impl Fn(&Proposal) -> bool,
    ) -> Result<Option<ProposalWritten>, WriteError> {
        let authored = |proposal: &Proposal| {
            proposal.destination == *destination
                && (edited_from.is_some()
                    || (proposal.created_by == call.actor.user
                        && proposal.proposing_agent == call.actor.agent))
                && fits(proposal)
        };
        if let Some(receipt) = self.store.receipt(patch_id).await? {
            let (kind, produced) = match edited_from {
                Some(base) => (EventType::ProposalEdited, base.next()),
                None => (EventType::ProposalCreated, Revision::NONE.next()),
            };
            let query = EventQuery {
                patch: Some(patch_id.clone()),
                size: PageSize::new(1),
                ..EventQuery::default()
            };
            let wrote = self.store.events(&query).await?;
            let written = wrote.items.first().and_then(|logged| {
                let event = &logged.event;
                let about =
                    event.event_type == kind && event.subject == Subject::Proposal(id.clone());
                about
                    .then(|| {
                        event.delta.iter().find_map(|write| match write {
                            Write::Put(Record::Proposal(proposal)) => Some(proposal),
                            _ => None,
                        })
                    })
                    .flatten()
            });
            let same = receipt.domain == *destination
                && receipt.revision == produced
                && written.is_some_and(authored);
            if !same {
                return Err(WriteError::Rejected(Rejection::PatchIdReused {
                    patch_id: patch_id.clone(),
                }));
            }
            patina_dst::reachable!("service-proposal-draft-answered-from-receipt");
            return Ok(Some(ProposalWritten::AlreadySaved { receipt }));
        }
        if edited_from.is_none()
            && let Some(held) = self.store.proposal(id).await?
            && authored(&held)
        {
            patina_dst::reachable!("service-proposal-draft-resubmitted");
            return Ok(Some(ProposalWritten::Existing { proposal: held }));
        }
        Ok(None)
    }

    /// H5: the answer for a proposal patch id already committed, if it was.
    async fn saved_before(
        &self,
        patch: &Patch,
    ) -> Result<Option<Result<ProposalWritten, WriteError>>, WriteError> {
        Ok(self.answer_from_receipt(patch).await?.map(|answer| {
            patina_dst::reachable!("service-proposal-patch-answered-from-receipt");
            answer.map(|written| ProposalWritten::AlreadySaved {
                receipt: written.receipt().clone(),
            })
        }))
    }

    /// Writes a patch to proposal `id` and answers with the proposal as committed.
    async fn write_proposal(
        &self,
        call: &Call,
        patch: &Patch,
        id: &ProposalId,
    ) -> Result<ProposalWritten, WriteError> {
        match self.write(call, patch, None).await? {
            Written::Applied { receipt, .. } => {
                let proposal = self.store.proposal(id).await?.ok_or_else(|| {
                    WriteError::Failed(ServiceError::Store(cairn_store::StoreError::Backend(
                        format!("proposal {id} was committed and cannot be read back"),
                    )))
                })?;
                Ok(ProposalWritten::Saved { receipt, proposal })
            }
            Written::AlreadyApplied { receipt } => Ok(ProposalWritten::AlreadySaved { receipt }),
        }
    }

    /// A domain's current revision; 0 when it does not exist (A17).
    pub(crate) async fn current_revision(&self, domain: &Domain) -> Result<Revision, ServiceError> {
        let target = match domain {
            Domain::Journey(id) => LoadTarget::Journey(id.clone()),
            Domain::Route(id) => LoadTarget::Route(id.clone()),
            Domain::Deployment => LoadTarget::Deployment,
        };
        Ok(self
            .store
            .load(&target)
            .await?
            .as_ref()
            .map_or(Revision::NONE, Document::revision))
    }
}

/// D7 for a deployment proposal: what its resolved mutations would newly cause in each
/// journey an entity merge in it was checked against.
fn merged(
    records: &Records,
    draft: &ProposalDraft,
    inputs: &ApplyInputs,
    settings: &crate::DeploymentSettings,
) -> BTreeMap<JourneyId, Consequences> {
    let Ok(mutations) = Mutations::new(resolve_partial(draft).mutations) else {
        return BTreeMap::new();
    };
    let patch = Patch {
        id: proposal_read_id(),
        target: PatchTarget::Deployment,
        base_revision: records.deployment.revision,
        deployment_revision: None,
        mutations,
    };
    match apply(records, &patch, inputs) {
        Ok(applied) => consequence::of(
            &patch.target,
            records,
            applied.records(),
            inputs.today,
            settings,
        ),
        Err(
            Rejection::Invalid { .. } | Rejection::Stale { .. } | Rejection::PatchIdReused { .. },
        ) => BTreeMap::new(),
    }
}

fn proposal_patch(
    patch_id: PatchId,
    destination: &Domain,
    id: &ProposalId,
    base_revision: Revision,
    mutation: Mutation,
) -> Patch {
    Patch {
        id: patch_id,
        target: PatchTarget::Proposal {
            id: id.clone(),
            destination: destination.clone(),
        },
        base_revision,
        deployment_revision: None,
        mutations: one(mutation),
    }
}

/// The patch target of a domain.
pub(crate) fn domain_target(domain: &Domain) -> PatchTarget {
    match domain {
        Domain::Journey(id) => PatchTarget::Journey(id.clone()),
        Domain::Route(id) => PatchTarget::Route(id.clone()),
        Domain::Deployment => PatchTarget::Deployment,
    }
}

fn one(mutation: Mutation) -> Mutations {
    match Mutations::new(vec![mutation]) {
        Ok(mutations) => mutations,
        Err(error) => unreachable!("one mutation is within the limit: {error}"),
    }
}

/// The id of a patch the service only reads with, never commits.
fn proposal_read_id() -> PatchId {
    match "p_proposal_preview".parse() {
        Ok(id) => id,
        Err(error) => unreachable!("a fixed patch id parses: {error}"),
    }
}
