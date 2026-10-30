//! A proposal written as a patch (I6, B8; ARCHITECTURE, Save as route): a patch that names a
//! proposal is added to it, or creates it, instead of being applied. A save as route is the
//! one content Cairn drafts: the engine drafts it from what the service loads, and the service
//! stores the draft as an ordinary proposal, reviewed and applied like any other (C14, I6,
//! I7). A proposal whose destination moved is refreshed (I6): a save is drafted again on the
//! new base with the reviewer's choices carried over by what they are about (identity, node),
//! anything else is re-based as it stands, and the edit gives it a new revision, so it must be
//! reviewed again before it can be applied. A write resubmitted under its patch id is answered
//! from its receipt before anything is drafted, since the destination may have moved since
//! (H5).
//!
//! Cost: a create or extend loads the proposal; a save also loads the journey, the deployment
//! and the route, then the engine's draft (a trial apply); one proposal write.

use std::collections::BTreeMap;
use std::fmt;

use cairn_engine::{ApplyInputs, DraftError};
use cairn_schema::{
    BoundedVec, Domain, DraftSource, JourneyId, Markdown, Mutation, NodeKey, PatchId, Proposal,
    ProposalDraft, ProposalId, Rejection, ReviewItem, Revision, RouteId, Title,
};

use crate::proposals::proposal_patch;
use crate::write::engine;
use crate::{Call, DomainPatch, ProposalWritten, Service, ServiceError, WriteError, load};

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
    /// The request is not one a proposal write takes; it names the argument at fault.
    Refused {
        /// The argument, by its name in the tool's and the API's request.
        path: String,
        /// What to change.
        message: String,
    },
    /// The proposal write was refused or failed.
    Write(WriteError),
}

impl From<DraftError> for ProposeError {
    fn from(error: DraftError) -> Self {
        match error {
            DraftError::JourneyMissing(_) | DraftError::RouteMissing(_) => {
                ProposeError::Missing(error)
            }
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
            ProposeError::Refused { path, message } => write!(formatter, "{path}: {message}"),
            ProposeError::Write(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ProposeError {}

impl ProposeError {
    fn refused(path: &str, message: impl Into<String>) -> Self {
        ProposeError::Refused {
            path: path.to_owned(),
            message: message.into(),
        }
    }
}

/// The proposal a patch is written to instead of being applied (I6): its id, and the editing
/// revision the writer saw when it exists already.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposing {
    id: ProposalId,
    revision: Option<Revision>,
}

impl Proposing {
    /// The proposal a request names, if it names one.
    ///
    /// # Errors
    ///
    /// [`ProposeError::Refused`] when it names a revision without a proposal.
    pub fn named(
        proposal: Option<ProposalId>,
        revision: Option<Revision>,
    ) -> Result<Option<Self>, ProposeError> {
        match (proposal, revision) {
            (Some(id), revision) => Ok(Some(Self { id, revision })),
            (None, None) => Ok(None),
            (None, Some(_)) => Err(ProposeError::refused(
                "proposal_revision",
                "a proposal revision belongs to a proposal: name it with proposal, or leave this out",
            )),
        }
    }

    /// The proposal's id.
    #[must_use]
    pub fn id(&self) -> &ProposalId {
        &self.id
    }
}

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
    /// I6: writes `submitted` to proposal `proposing` instead of applying it, as `call`'s actor.
    /// The proposal is created when it does not exist, open, against the patch's base
    /// revision (0 creates the destination), titled by the patch's note; when it does, the
    /// patch's mutations are added to it, against the proposal's editing revision and the
    /// destination revision it was drafted against. A save as route (an `open_draft` from a
    /// journey, after the `create_route` when the route does not exist) is the whole content
    /// of a create, drafted by Cairn with a participation item per explicit identity and an
    /// exclusion item per node for the reviewer (B8).
    ///
    /// # Errors
    ///
    /// [`ProposeError::Refused`] when the request is not one a proposal write takes;
    /// [`ProposeError::Missing`] when a save's journey does not exist,
    /// [`ProposeError::CannotDraft`] when its draft is too large; [`ProposeError::Write`]
    /// when the proposal write is refused (stale, reused patch id, not open) or fails.
    pub async fn propose(
        &self,
        call: &Call,
        submitted: &DomainPatch,
        proposing: &Proposing,
    ) -> Result<ProposalWritten, ProposeError> {
        let patch = submitted.patch();
        let destination = patch.target.domain();
        let id = &proposing.id;
        if patch.deployment_revision.is_some() {
            return Err(ProposeError::refused(
                "deployment_revision",
                "a proposal records one revision, its destination's: leave deployment_revision out",
            ));
        }
        let mutations = patch.mutations.as_slice();
        let save = save_of(&destination, mutations)?;
        let Some(revision) = proposing.revision else {
            // A create, or its resubmission after a lost answer.
            return self
                .create_proposed(call, submitted, &destination, id, save)
                .await;
        };
        let Some(held) = self.store.proposal(id).await? else {
            return Err(ProposeError::refused(
                "proposal_revision",
                format!(
                    "there is no proposal {id} to add to: leave proposal_revision out to create it"
                ),
            ));
        };
        if save.is_some()
            || matches!(
                drafted(&held.destination, &held.draft),
                Drafted::Save { .. }
            )
        {
            return Err(ProposeError::refused(
                "mutations",
                "a save as route is a proposal of its own: it takes nothing added, and nothing is added to it",
            ));
        }
        self.addressed(&destination, id).await?;
        let extend = Mutation::ExtendProposal {
            destination_revision: patch.base_revision,
            mutations: BoundedVec::new(mutations.to_vec())
                .map_err(|error| ProposeError::refused("mutations", error.to_string()))?,
        };
        let edit = proposal_patch(patch.id.clone(), &destination, id, revision, extend);
        if let Some(answer) = self.saved_before(&edit).await? {
            return Ok(answer?);
        }
        Ok(self
            .write_proposal(call, &edit, id, submitted.note().cloned())
            .await?)
    }

    /// A create of proposal `id`: the patch's own mutations, or Cairn's draft of the save.
    async fn create_proposed(
        &self,
        call: &Call,
        submitted: &DomainPatch,
        destination: &Domain,
        id: &ProposalId,
        save: Option<(JourneyId, RouteId, Option<Title>)>,
    ) -> Result<ProposalWritten, ProposeError> {
        let patch = submitted.patch();
        let is_save = save.is_some();
        let draft = if let Some((journey, route, name)) = save {
            if let Some(answer) = self
                .drafted_before(call, &patch.id, (destination, id), None, |held| {
                    asked(
                        &Drafted::Save {
                            journey: journey.clone(),
                            route: route.clone(),
                            name: name.clone(),
                        },
                        &drafted(&held.destination, &held.draft),
                    ) && held.draft.destination_revision == patch.base_revision
                })
                .await?
            {
                return Ok(answer);
            }
            let draft = self
                .draft_save(call, &journey, &route, name.as_ref())
                .await?;
            if draft.destination_revision != patch.base_revision {
                return Err(self
                    .stale_save(destination, &draft, patch.base_revision)
                    .await?);
            }
            draft
        } else {
            let title = submitted.note().map(title_of).transpose()?.ok_or_else(|| {
                ProposeError::refused(
                    "note",
                    "a new proposal is titled by the note: say in a line what it proposes",
                )
            })?;
            ProposalDraft::new(
                title,
                None,
                patch.base_revision,
                BoundedVec::new(patch.mutations.as_slice().to_vec())
                    .map_err(|error| ProposeError::refused("mutations", error.to_string()))?,
                BoundedVec::default(),
            )
            .map_err(|error| ProposeError::refused("mutations", error.to_string()))?
        };
        if !is_save
            && let Some(answer) = self
                .create_answered(submitted, destination, id, &draft)
                .await?
        {
            return Ok(answer);
        }
        Ok(self
            .put_new_proposal(call, patch.id.clone(), destination, id, draft)
            .await?)
    }

    /// What a create of `draft` as proposal `id` is already answered by: its receipt, whatever
    /// the proposal has become since a lost answer; else a refusal when `id` holds other
    /// content, since any other write to a held id is an addition and says so.
    async fn create_answered(
        &self,
        submitted: &DomainPatch,
        destination: &Domain,
        id: &ProposalId,
        draft: &ProposalDraft,
    ) -> Result<Option<ProposalWritten>, ProposeError> {
        let patch = submitted.patch();
        let created = proposal_patch(
            patch.id.clone(),
            destination,
            id,
            Revision::NONE,
            Mutation::CreateProposal {
                proposal: draft.clone(),
            },
        );
        if let Some(answer) = self.saved_before(&created).await? {
            return Ok(Some(answer?));
        }
        match self.store.proposal(id).await? {
            Some(held)
                if !same_content(
                    &held,
                    destination,
                    patch.base_revision,
                    patch.mutations.as_slice(),
                ) =>
            {
                Err(ProposeError::refused(
                    "proposal_revision",
                    format!("proposal {id} exists: name its editing revision to add to it"),
                ))
            }
            _ => Ok(None),
        }
    }

    /// The stale rejection for a save drafted at the destination's current revision when the
    /// request was drafted at `base`.
    async fn stale_save(
        &self,
        destination: &Domain,
        draft: &ProposalDraft,
        base: Revision,
    ) -> Result<ProposeError, ServiceError> {
        let stale = self
            .stale_base(destination, base, draft.destination_revision)
            .await?;
        let Some(stale) = stale else {
            unreachable!("the revisions differ");
        };
        Ok(ProposeError::Write(WriteError::Rejected(
            Rejection::Stale {
                conflicts: vec![stale.conflict],
                intervening: stale.intervening,
            },
        )))
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
                self.draft_save(call, &journey, &route, name.as_ref())
                    .await?
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
        name: Option<&Title>,
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

/// The journey, route and route name of a save as route a patch asks for: `open_draft` from a
/// journey, after the `create_route` that names a route not yet there. None when the patch
/// holds no save.
fn save_of(
    destination: &Domain,
    mutations: &[Mutation],
) -> Result<Option<(JourneyId, RouteId, Option<Title>)>, ProposeError> {
    let (name, journey) = match mutations {
        [
            Mutation::CreateRoute {
                name,
                description: None,
            },
            Mutation::OpenDraft {
                source: DraftSource::SaveAsRoute { journey },
            },
        ] => (Some(name), journey),
        [
            Mutation::OpenDraft {
                source: DraftSource::SaveAsRoute { journey },
            },
        ] => (None, journey),
        _ if mutations.iter().any(|mutation| {
            matches!(
                mutation,
                Mutation::OpenDraft {
                    source: DraftSource::SaveAsRoute { .. }
                }
            )
        }) =>
        {
            return Err(ProposeError::refused(
                "mutations",
                "a save as route is an open_draft from a journey, after a create_route that names the route when it does not exist yet (its description is the journey's), and nothing else",
            ));
        }
        _ => return Ok(None),
    };
    let Domain::Route(route) = destination else {
        return Err(ProposeError::refused(
            "target",
            "a save as route is written to a route",
        ));
    };
    Ok(Some((journey.clone(), route.clone(), name.cloned())))
}

/// Whether proposal `held` is what a create under its id asks for: the same destination,
/// base revision and mutations (a create resubmitted after a lost answer).
fn same_content(
    held: &Proposal,
    destination: &Domain,
    base: Revision,
    mutations: &[Mutation],
) -> bool {
    held.destination == *destination
        && held.draft.destination_revision == base
        && held.draft.mutations.as_slice() == mutations
}

/// A note as a proposal's title: one line, a title's size.
fn title_of(note: &Markdown) -> Result<Title, ProposeError> {
    note.as_str()
        .trim()
        .parse()
        .map_err(|error: cairn_schema::TextError| ProposeError::refused("note", error.to_string()))
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
