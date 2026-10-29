//! Save as route as a proposal (B8; ARCHITECTURE, Save as route): the engine drafts it from
//! what the service loads, and the service stores the draft as an ordinary proposal, reviewed
//! and applied like any other (C14, I6, I7). A proposal whose destination moved is refreshed
//! (I6): a save is drafted again on the new base with the reviewer's choices carried over by
//! what they are about (identity, node), anything else is re-based as it stands, and the edit gives it a new revision, so it must be reviewed again before it can be
//! applied. A write resubmitted under its patch id is answered from its receipt before
//! anything is drafted, since the destination may have moved since (H5), and a proposal
//! resubmitted under a new patch id is the one its author already holds (I6).
//!
//! Cost: one load of the journey, the deployment, and the route, then the engine's draft (a
//! trial apply), and one proposal write.

use std::collections::BTreeMap;
use std::fmt;

use cairn_engine::{ApplyInputs, DraftError};
use cairn_schema::{
    Domain, DraftSource, JourneyId, Mutation, NodeKey, PatchId, ProposalDraft, ProposalId,
    ReviewItem, Revision, RouteId, Title,
};

use crate::write::engine;
use crate::{Call, ProposalWritten, Service, ServiceError, WriteError, load};

/// A proposal the service could not draft or store, classified once here so every host
/// answers it the same way: something it names is missing, the caller asked for a draft that
/// cannot be made, or the write was refused or failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposeError {
    /// What the draft reads does not exist: the journey (B8).
    Missing(DraftError),
    /// The engine cannot draft it from what exists, which the caller can change: the draft is
    /// too large (B8).
    CannotDraft(DraftError),
    /// The proposal write was refused or failed.
    Write(WriteError),
}

impl From<DraftError> for ProposeError {
    fn from(error: DraftError) -> Self {
        match error {
            DraftError::JourneyMissing(_) => ProposeError::Missing(error),
            DraftError::TooLarge => ProposeError::CannotDraft(error),
        }
    }
}

impl From<WriteError> for ProposeError {
    fn from(error: WriteError) -> Self {
        ProposeError::Write(error)
    }
}

impl From<ServiceError> for ProposeError {
    fn from(error: ServiceError) -> Self {
        ProposeError::Write(WriteError::Failed(error))
    }
}

impl From<cairn_store::StoreError> for ProposeError {
    fn from(error: cairn_store::StoreError) -> Self {
        ProposeError::Write(error.into())
    }
}

impl fmt::Display for ProposeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProposeError::Missing(error) | ProposeError::CannotDraft(error) => error.fmt(formatter),
            ProposeError::Write(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ProposeError {}

/// What a stored proposal was drafted as, read off its mutations.
#[derive(Debug, PartialEq, Eq)]
enum Drafted {
    Save {
        journey: JourneyId,
        route: RouteId,
        name: Option<Title>,
    },
    Other,
}

impl Service {
    /// B8: proposes saving `journey`'s structure as a draft of `route` (created with `name`
    /// when it does not exist), as proposal `id`, with a participation item per explicit
    /// identity and an exclusion item per node for the reviewer.
    ///
    /// # Errors
    ///
    /// [`ProposeError::Missing`] when the journey does not exist, [`ProposeError::CannotDraft`]
    /// when the draft is too large; [`ProposeError::Write`] when the proposal write is refused
    /// or fails.
    pub async fn propose_save_as_route(
        &self,
        call: &Call,
        patch_id: PatchId,
        id: &ProposalId,
        journey: &JourneyId,
        route: &RouteId,
        name: &Title,
    ) -> Result<ProposalWritten, ProposeError> {
        let destination = Domain::Route(route.clone());
        if let Some(answer) = self
            .drafted_before(call, &patch_id, (&destination, id), None, |held| {
                asked(
                    &Drafted::Save {
                        journey: journey.clone(),
                        route: route.clone(),
                        name: Some(name.clone()),
                    },
                    &drafted(&held.destination, &held.draft),
                )
            })
            .await?
        {
            return Ok(answer);
        }
        let draft = self.draft_save(call, journey, route, name).await?;
        Ok(self
            .create_proposal(call, patch_id, &destination, id, draft)
            .await?)
    }

    /// I6: refreshes proposal `id` against its destination as it stands now, against
    /// `base_revision`, its editing revision the caller saw. A save as route is drafted again
    /// on the new base, carrying each choice the reviewer made to the item about the same thing
    /// (an identity's mapping, a node's exclusion); any other proposal keeps its mutations and items
    /// and names the current revision. Its title and description stay as edited. The edit
    /// moves the proposal's revision, so it must be reviewed again before it applies.
    ///
    /// # Errors
    ///
    /// [`WriteError::ProposalMissing`] when there is no such proposal under `destination`;
    /// [`ProposeError::Missing`] or [`ProposeError::CannotDraft`] as for drafting it; [`ProposeError::Write`] when the edit is
    /// stale, the proposal is not open, or the write fails.
    pub async fn refresh_proposal(
        &self,
        call: &Call,
        patch_id: PatchId,
        destination: &Domain,
        id: &ProposalId,
        base_revision: Revision,
    ) -> Result<ProposalWritten, ProposeError> {
        let held = self
            .proposal_at(destination, id)
            .await?
            .ok_or_else(|| WriteError::ProposalMissing(id.clone()))?;
        if let Some(answer) = self
            .drafted_before(
                call,
                &patch_id,
                (&held.destination, id),
                Some(base_revision),
                |_| true,
            )
            .await?
        {
            return Ok(answer);
        }
        let mut draft = match drafted(&held.destination, &held.draft) {
            Drafted::Save {
                journey,
                route,
                name,
            } => {
                let name = match name {
                    Some(name) => name,
                    None => match self.route(&route).await? {
                        Some(found) => found.header.name,
                        None => held.draft.title.clone(),
                    },
                };
                self.draft_save(call, &journey, &route, &name).await?
            }
            Drafted::Other => ProposalDraft {
                destination_revision: self.current_revision(&held.destination).await?,
                ..held.draft.clone()
            },
        };
        carry(held.draft.items.as_slice(), draft.items.as_mut_slice());
        draft.title = held.draft.title.clone();
        draft.description = held.draft.description.clone();
        patina_dst::reachable!("service-proposal-refreshed-against-destination");
        Ok(self
            .edit_proposal(call, patch_id, &held.destination, id, base_revision, draft)
            .await?)
    }

    async fn draft_save(
        &self,
        call: &Call,
        journey: &JourneyId,
        route: &RouteId,
        name: &Title,
    ) -> Result<ProposalDraft, ProposeError> {
        let records = load::drafting(&*self.store, journey, route).await?;
        let inputs = self.drafting_inputs(call);
        engine(&Domain::Route(route.clone()), || {
            cairn_engine::save_as_route(&records, journey, route, name, &inputs)
        })?
        .map_err(ProposeError::from)
    }

    /// The apply inputs a draft's trial apply runs with: `call`'s actor and today.
    fn drafting_inputs(&self, call: &Call) -> ApplyInputs {
        ApplyInputs {
            today: self.settings.today(call.now),
            at: call.now,
            actor: call.actor.clone(),
            note: None,
        }
    }
}

/// What `draft`, a proposal for `destination`, was drafted as: a save of a journey into the
/// route it is for, or something else.
fn drafted(destination: &Domain, draft: &ProposalDraft) -> Drafted {
    let mutations = draft.mutations.as_slice();
    match destination {
        Domain::Route(route) => {
            let name = mutations.iter().find_map(|mutation| match mutation {
                Mutation::CreateRoute { name, .. } => Some(name.clone()),
                _ => None,
            });
            mutations
                .iter()
                .find_map(|mutation| match mutation {
                    Mutation::OpenDraft {
                        source: DraftSource::SaveAsRoute { journey },
                    } => Some(Drafted::Save {
                        journey: journey.clone(),
                        route: route.clone(),
                        name: name.clone(),
                    }),
                    _ => None,
                })
                .unwrap_or(Drafted::Other)
        }
        Domain::Journey(_) | Domain::Deployment => Drafted::Other,
    }
}

/// H5: whether a stored proposal drafted as `stored` is what `request` asks for: the same
/// saved journey and route (a save's name is in the draft only when it created the route).
fn asked(request: &Drafted, stored: &Drafted) -> bool {
    match (request, stored) {
        (
            Drafted::Save {
                journey,
                route,
                name,
            },
            Drafted::Save {
                journey: saved,
                route: into,
                name: named,
            },
        ) => journey == saved && route == into && (named.is_none() || named == name),
        (Drafted::Other, _) => false,
        _ => request == stored,
    }
}

/// What a review item is about, for carrying a choice made on it to a refreshed draft.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum About {
    Identity(cairn_schema::IdentityKey),
    Exclusion(NodeKey),
}

/// The item's subject, when it holds a choice.
fn about(item: &ReviewItem) -> Option<About> {
    Some(match item {
        ReviewItem::Participation { identity, .. } => About::Identity(identity.clone()),
        ReviewItem::Exclusion { node, .. } => About::Exclusion(node.clone()),
        ReviewItem::Cascade { .. } | ReviewItem::Violation { .. } => {
            return None;
        }
    })
}

/// I6: carries each choice in `old` to the item in `new` about the same thing: an identity's
/// mapping and a node's exclusion. An item with no counterpart keeps the draft's default.
fn carry(old: &[ReviewItem], new: &mut [ReviewItem]) {
    let chosen: BTreeMap<About, &ReviewItem> = old
        .iter()
        .filter_map(|item| about(item).map(|subject| (subject, item)))
        .collect();
    for item in new.iter_mut() {
        let Some(previous) = about(item).and_then(|subject| chosen.get(&subject)) else {
            continue;
        };
        match (item, previous) {
            (
                ReviewItem::Participation { mapping, .. },
                ReviewItem::Participation {
                    mapping: Some(made),
                    ..
                },
            ) => *mapping = Some(made.clone()),
            (
                ReviewItem::Exclusion { excluded, .. },
                ReviewItem::Exclusion { excluded: was, .. },
            ) => *excluded = *was,
            _ => {}
        }
    }
}
