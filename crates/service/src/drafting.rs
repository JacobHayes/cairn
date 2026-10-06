//! Upgrade, save as route, and re-link as proposals (B7, B8, B9; ARCHITECTURE, Upgrade,
//! save-as-route, re-link): the engine drafts each from what the service loads, and the
//! service stores the draft as an ordinary proposal, reviewed and applied like any other
//! (C14, I6, I7). A proposal whose destination moved is refreshed (I6): an upgrade, save, or
//! re-link is drafted again on the new base with the reviewer's choices carried over by what
//! they are about (node and field, edge, entity, role, kind), anything else is re-based as it
//! stands, and the edit gives it a new revision, so it must be reviewed again before it can be
//! applied. A write resubmitted under its patch id is answered from its receipt before
//! anything is drafted, since the destination may have moved since (H5), and a proposal
//! resubmitted under a new patch id is the one its author already holds (I6).
//!
//! Cost: one load of the journey, the deployment, the route, and the versions the draft
//! reads, then the engine's draft (a merge and a trial apply for an upgrade or a save, a pass
//! over the nodes for a re-link), and one proposal write.

use std::collections::BTreeMap;
use std::fmt;

use cairn_engine::{ApplyInputs, DraftError};
use cairn_schema::{
    AttachmentKey, Conflict, Domain, DraftSource, Edge, JourneyId, KindKey, Lineage, Mutation,
    NodeField, NodeKey, PatchId, ProposalDraft, ProposalId, ReviewItem, Revision, RoleKey, RouteId,
    Title, VersionNumber,
};
use cairn_store::Store;

use crate::write::engine;
use crate::{Call, ProposalWritten, Service, ServiceError, WriteError, load};

/// A proposal the service could not draft or store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposeError {
    /// The engine cannot draft it from what is stored: the journey or a version does not
    /// exist, the journey follows no route, the version is not newer, or the draft is too
    /// large (B7, B8, B9).
    Draft(DraftError),
    /// No such proposal to refresh.
    ProposalMissing(ProposalId),
    /// The proposal write was refused or failed.
    Write(WriteError),
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
            ProposeError::Draft(error) => error.fmt(formatter),
            ProposeError::ProposalMissing(id) => write!(formatter, "no proposal {id}"),
            ProposeError::Write(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ProposeError {}

/// What a stored proposal was drafted as, read off its mutations.
#[derive(Debug, PartialEq, Eq)]
enum Drafted {
    Upgrade(JourneyId, VersionNumber),
    Relink(JourneyId, Lineage),
    Save {
        journey: JourneyId,
        route: RouteId,
        name: Option<Title>,
    },
    Other,
}

impl<S: Store> Service<S> {
    /// B7: proposes upgrading `journey` to version `to` of the route it follows, as proposal
    /// `id`: the merge's clean outcomes ride in its upgrade mutation, and its conflicts, kept
    /// edits, orphans, and the violations a trial apply finds are its review items. Nothing
    /// changes in the journey until someone applies it (Journey durability).
    ///
    /// # Errors
    ///
    /// [`ProposeError::Draft`] when the journey or a version does not exist, the journey
    /// follows no route, or `to` is older; [`ProposeError::Write`] when the proposal write is
    /// refused or fails.
    pub async fn propose_upgrade(
        &self,
        call: &Call,
        patch_id: PatchId,
        id: &ProposalId,
        journey: &JourneyId,
        to: VersionNumber,
    ) -> Result<ProposalWritten, ProposeError> {
        let destination = Domain::Journey(journey.clone());
        if let Some(answer) = self
            .drafted_before(call, &patch_id, (&destination, id), None, |held| {
                asked(
                    &Drafted::Upgrade(journey.clone(), to),
                    &drafted(&held.destination, &held.draft),
                )
            })
            .await?
        {
            return Ok(answer);
        }
        let draft = self.draft_upgrade(call, journey, to).await?;
        Ok(self
            .create_proposal(call, patch_id, &destination, id, draft)
            .await?)
    }

    /// B8: proposes saving `journey`'s structure as a draft of `route` (created with `name`
    /// when it does not exist), as proposal `id`, with a participation item per explicit
    /// entity and an exclusion item per node for the reviewer.
    ///
    /// # Errors
    ///
    /// As [`Service::propose_upgrade`].
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

    /// B9: proposes re-linking `journey` to `lineage`, a version its saved route published, as
    /// proposal `id`: each difference from the version is kept as a local edit unless the
    /// reviewer takes the route's value. The old lineage holds until it is applied.
    ///
    /// # Errors
    ///
    /// As [`Service::propose_upgrade`].
    pub async fn propose_relink(
        &self,
        call: &Call,
        patch_id: PatchId,
        id: &ProposalId,
        journey: &JourneyId,
        lineage: &Lineage,
    ) -> Result<ProposalWritten, ProposeError> {
        let destination = Domain::Journey(journey.clone());
        if let Some(answer) = self
            .drafted_before(call, &patch_id, (&destination, id), None, |held| {
                asked(
                    &Drafted::Relink(journey.clone(), lineage.clone()),
                    &drafted(&held.destination, &held.draft),
                )
            })
            .await?
        {
            return Ok(answer);
        }
        let draft = self.draft_relink(journey, lineage).await?;
        Ok(self
            .create_proposal(call, patch_id, &destination, id, draft)
            .await?)
    }

    /// I6: refreshes proposal `id` against its destination as it stands now, against
    /// `base_revision`, its editing revision the caller saw. An upgrade, save as route, or
    /// re-link is drafted again on the new base, carrying each choice the reviewer made to the
    /// item about the same thing (a conflict by node and field, edge, participation, resource,
    /// role, kind, or default owner, when the new item still offers it; an orphan's keep, an
    /// entity's mapping, a node's exclusion); any other proposal keeps its mutations and items
    /// and names the current revision. Its title and description stay as edited. The edit
    /// moves the proposal's revision, so it must be reviewed again before it applies.
    ///
    /// # Errors
    ///
    /// [`ProposeError::ProposalMissing`] when there is no such proposal;
    /// [`ProposeError::Draft`] as for drafting it; [`ProposeError::Write`] when the edit is
    /// stale, the proposal is not open, or the write fails.
    pub async fn refresh_proposal(
        &self,
        call: &Call,
        patch_id: PatchId,
        id: &ProposalId,
        base_revision: Revision,
    ) -> Result<ProposalWritten, ProposeError> {
        let held = self
            .store
            .proposal(id)
            .await?
            .ok_or_else(|| ProposeError::ProposalMissing(id.clone()))?;
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
            Drafted::Upgrade(journey, to) => self.draft_upgrade(call, &journey, to).await?,
            Drafted::Relink(journey, lineage) => self.draft_relink(&journey, &lineage).await?,
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
        patina_dst::reachable!("service: a proposal refreshed against its destination");
        Ok(self
            .edit_proposal(call, patch_id, &held.destination, id, base_revision, draft)
            .await?)
    }

    async fn draft_upgrade(
        &self,
        call: &Call,
        journey: &JourneyId,
        to: VersionNumber,
    ) -> Result<ProposalDraft, ProposeError> {
        let records = load::drafting(&*self.store, journey, None, |held| {
            let follows = held.and_then(|held| held.header.lineage.clone());
            follows
                .into_iter()
                .flat_map(|current| {
                    let target = Lineage {
                        route: current.route.clone(),
                        version: to,
                    };
                    [current, target]
                })
                .collect()
        })
        .await?;
        let inputs = self.drafting_inputs(call);
        engine(&Domain::Journey(journey.clone()), || {
            cairn_engine::upgrade(&records, journey, to, &inputs)
        })?
        .map_err(ProposeError::Draft)
    }

    async fn draft_save(
        &self,
        call: &Call,
        journey: &JourneyId,
        route: &RouteId,
        name: &Title,
    ) -> Result<ProposalDraft, ProposeError> {
        let records = load::drafting(&*self.store, journey, Some(route), |_| Vec::new()).await?;
        let inputs = self.drafting_inputs(call);
        engine(&Domain::Route(route.clone()), || {
            cairn_engine::save_as_route(&records, journey, route, name, &inputs)
        })?
        .map_err(ProposeError::Draft)
    }

    async fn draft_relink(
        &self,
        journey: &JourneyId,
        lineage: &Lineage,
    ) -> Result<ProposalDraft, ProposeError> {
        let records =
            load::drafting(&*self.store, journey, None, |_| vec![lineage.clone()]).await?;
        engine(&Domain::Journey(journey.clone()), || {
            cairn_engine::relink(&records, journey, lineage)
        })?
        .map_err(ProposeError::Draft)
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

/// What `draft`, a proposal for `destination`, was drafted as: an upgrade or re-link of the
/// journey it is for, a save of a journey into the route it is for, or something else.
fn drafted(destination: &Domain, draft: &ProposalDraft) -> Drafted {
    let mutations = draft.mutations.as_slice();
    match destination {
        Domain::Journey(journey) => mutations
            .iter()
            .find_map(|mutation| match mutation {
                Mutation::Upgrade { to } => Some(Drafted::Upgrade(journey.clone(), *to)),
                Mutation::Relink { lineage } => {
                    Some(Drafted::Relink(journey.clone(), lineage.clone()))
                }
                _ => None,
            })
            .unwrap_or(Drafted::Other),
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
        Domain::Deployment => Drafted::Other,
    }
}

/// H5: whether a stored proposal drafted as `stored` is what `request` asks for: the same
/// upgrade target, re-link lineage, or saved journey and route (a save's name is in the draft
/// only when it created the route).
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
    Field(NodeKey, NodeField),
    Edge(Edge),
    Participation(NodeKey, KindKey),
    Resource(NodeKey, AttachmentKey),
    Shape(NodeKey),
    Answer(NodeKey),
    Role(RoleKey),
    Kind(KindKey),
    DefaultOwner,
    Orphan(NodeKey),
    Entity(cairn_schema::EntityKey),
    Exclusion(NodeKey),
}

/// The item's subject, when it holds a choice.
fn about(item: &ReviewItem) -> Option<About> {
    Some(match item {
        ReviewItem::Conflict { conflict, .. } => match conflict {
            Conflict::Field { node, journey, .. } => About::Field(node.clone(), journey.field()),
            Conflict::Edge { edge, .. } => About::Edge(edge.clone()),
            Conflict::Participation { node, kind, .. } => {
                About::Participation(node.clone(), kind.clone())
            }
            Conflict::Resource { node, resource, .. } => {
                About::Resource(node.clone(), resource.clone())
            }
            Conflict::Shape { journey, .. } => About::Shape(journey.key.clone()),
            Conflict::Answer { decision, .. } => About::Answer(decision.clone()),
            Conflict::Role { role, .. } => About::Role(role.clone()),
            Conflict::Kind { kind, .. } => About::Kind(kind.clone()),
            Conflict::DefaultOwner { .. } => About::DefaultOwner,
        },
        ReviewItem::Orphan { node, .. } => About::Orphan(node.clone()),
        ReviewItem::Participation { entity, .. } => About::Entity(entity.clone()),
        ReviewItem::Exclusion { node, .. } => About::Exclusion(node.clone()),
        ReviewItem::KeptLocalEdit { .. }
        | ReviewItem::Cascade { .. }
        | ReviewItem::Violation { .. } => {
            return None;
        }
    })
}

/// I6: carries each choice in `old` to the item in `new` about the same thing: a conflict's
/// resolution when the new conflict offers it, an orphan's keep, an entity's mapping, and a
/// node's exclusion. An item with no counterpart keeps the draft's default.
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
                ReviewItem::Conflict {
                    conflict,
                    resolution,
                },
                ReviewItem::Conflict {
                    resolution: Some(made),
                    ..
                },
            ) if conflict.offers(made) => *resolution = Some(made.clone()),
            (ReviewItem::Orphan { keep, .. }, ReviewItem::Orphan { keep: kept, .. }) => {
                *keep = *kept;
            }
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
