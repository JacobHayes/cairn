//! Domain patches (A17; ARCHITECTURE, Service layer and composition): receipt first, load,
//! apply, consequences, commit, notify. A stale domain patch is decided here, once, for every
//! surface and both hosts (H5): answered stale, with what moved and what intervened, and
//! never applied past other people's changes. The writer reads again and decides again.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

use cairn_engine::{Applied, ApplyInputs, Records, apply};
use cairn_schema::{
    Consequences, Domain, EventType, IdentityKey, JourneyId, Location, Markdown, Mutation, Notice,
    Patch, PatchReceipt, PatchTarget, ProposalId, Record, Rejection, Revision, RevisionOf, Subject,
    Violation, ViolationCode, Violations, When, Write,
};
use cairn_store::{Commit, CommitError, Committed, EventQuery, PageSize};

use crate::{Call, Service, ServiceError, consequence, load, notices};

/// A patch to a journey, a route with its draft, or the deployment (A17), with the note its
/// events carry (J1). A patch to a proposal goes through the proposal's own operations,
/// so it is not a domain patch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainPatch {
    patch: Patch,
    note: Option<Markdown>,
}

impl DomainPatch {
    /// The patch, with an optional note for each of its events.
    ///
    /// # Errors
    ///
    /// [`NotADomainPatch`] when it targets a proposal.
    pub fn new(patch: Patch, note: Option<Markdown>) -> Result<Self, NotADomainPatch> {
        if let PatchTarget::Proposal { .. } = patch.target {
            return Err(NotADomainPatch);
        }
        Ok(Self { patch, note })
    }

    /// The patch.
    #[must_use]
    pub fn patch(&self) -> &Patch {
        &self.patch
    }

    /// The note its events carry.
    #[must_use]
    pub fn note(&self) -> Option<&Markdown> {
        self.note.as_ref()
    }
}

/// The patch targets a proposal, which is edited through its own lifecycle, not as a domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotADomainPatch;

impl fmt::Display for NotADomainPatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a proposal is edited through its own operations, not as a domain")
    }
}

impl std::error::Error for NotADomainPatch {}

/// What an accepted domain patch answers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Written {
    /// Committed now: the receipt, and what it newly caused in each journey it changed or
    /// whose meaning it changed (D7; a journey with nothing new is left out).
    Applied {
        /// The receipt: patch id, domain, content hash, and the revision produced.
        receipt: PatchReceipt,
        /// D7, by journey.
        consequences: BTreeMap<JourneyId, Consequences>,
        /// A20: the notices of the graph a route import or publish leaves; none for any
        /// other patch. Advisory: the patch was accepted.
        notices: Vec<Notice>,
    },
    /// The patch id was committed before with the same content (H5): answered from its
    /// receipt, with no consequences.
    AlreadyApplied {
        /// The original receipt.
        receipt: PatchReceipt,
    },
}

impl Written {
    /// The receipt, whichever way it went.
    #[must_use]
    pub fn receipt(&self) -> &PatchReceipt {
        match self {
            Written::Applied { receipt, .. } | Written::AlreadyApplied { receipt } => receipt,
        }
    }
}

/// A patch that was not committed. Either way nothing was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteError {
    /// The patch is stale (with what intervened), reuses a patch id, or is invalid (every
    /// violation): the engine's and the store's rejection shapes (A15, H5).
    Rejected(Rejection),
    /// I6: a write to a proposal that is not under the destination it was addressed to,
    /// another destination's included, is missing, never rejected. Only a proposal's own
    /// operations answer it.
    ProposalMissing(ProposalId),
    /// The service or the store failed.
    Failed(ServiceError),
}

impl From<ServiceError> for WriteError {
    fn from(error: ServiceError) -> Self {
        WriteError::Failed(error)
    }
}

impl From<cairn_store::StoreError> for WriteError {
    fn from(error: cairn_store::StoreError) -> Self {
        WriteError::Failed(ServiceError::Store(error))
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteError::Rejected(rejection) => write!(formatter, "rejected: {rejection:?}"),
            WriteError::ProposalMissing(id) => write!(formatter, "no proposal {id}"),
            WriteError::Failed(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for WriteError {}

impl Service {
    /// A17: applies a domain patch as `call`'s actor (H2), at `call`'s today in the
    /// deployment's zone, and commits it. In order: a patch id already committed is answered
    /// from its receipt before anything is loaded (H5); a save as route, which changes
    /// nothing until confirmed through its proposal, is refused (B8, I7); the
    /// records it reads, including any proposal it applies, are loaded;
    /// `apply` validates it (A15, D4); its consequences are derived (D7); the commit rechecks
    /// every revision it read (H5, E6); and every revision the commit moved is announced
    /// (H6). A patch whose named revisions moved is answered stale (H5).
    ///
    /// # Errors
    ///
    /// [`WriteError::Rejected`] for a stale, reused, or invalid patch;
    /// [`WriteError::Failed`] when the store fails or the engine panics.
    pub async fn patch(&self, call: &Call, submitted: &DomainPatch) -> Result<Written, WriteError> {
        let patch = submitted.patch();
        if let Some(answer) = self.answer_from_receipt(patch).await? {
            patina_dst::reachable!("service-resubmission-answered-from-receipt");
            return answer;
        }
        self.write(call, patch, submitted.note.clone()).await
    }

    // PRD: H5.
    /// The write path after the receipt lookup, for a domain patch and a patch to a proposal
    /// alike: load, apply, consequences, commit, announce.
    ///
    /// A patch whose named revisions moved is answered stale, with what the commits since the
    /// revisions it names changed. The intervening events are read after the load whose records
    /// are applied, so they cover every commit up to it. A commit that loses to another is
    /// stale too.
    pub(crate) async fn write(
        &self,
        call: &Call,
        patch: &Patch,
        note: Option<Markdown>,
    ) -> Result<Written, WriteError> {
        let today = self.settings.today(call.now);
        let domain = patch.target.domain();
        let loaded = load::records(&*self.store, patch).await?;
        let inputs = ApplyInputs {
            today,
            at: call.now,
            actor: call.actor.clone(),
            note,
        };
        let wrong_offsets = self.wrong_offsets(patch);
        let applied = match engine(&domain, || apply(&loaded.records, patch, &inputs))? {
            Ok(applied) => match wrong_offsets {
                None => applied,
                Some(violations) => {
                    return self
                        .rejected(patch, Rejection::Invalid { violations })
                        .await;
                }
            },
            // Every violation at once: the wrong offsets beside what the engine found.
            Err(Rejection::Invalid { violations }) if wrong_offsets.is_some() => {
                let all = wrong_offsets
                    .iter()
                    .flat_map(cairn_schema::Violations::as_slice)
                    .chain(violations.as_slice())
                    .cloned()
                    .collect();
                return match Violations::new(all) {
                    Ok(violations) => {
                        self.rejected(patch, Rejection::Invalid { violations })
                            .await
                    }
                    Err(_) => Err(WriteError::Rejected(Rejection::Invalid { violations })),
                };
            }
            Err(Rejection::Stale { conflicts, .. }) => {
                // After the load, so that this covers every commit up to it.
                let intervening = self.store.intervening(&conflicts).await?;
                return self
                    .rejected(
                        patch,
                        Rejection::Stale {
                            conflicts,
                            intervening,
                        },
                    )
                    .await;
            }
            Err(other) => return self.rejected(patch, other).await,
        };
        let (caused, noticed) = engine(&domain, || {
            let caused = consequence::of(
                &patch.target,
                &loaded.records,
                applied.records(),
                call.now,
                &self.settings,
            );
            (caused, notices::of(patch, &loaded.records, &applied))
        })?;
        self.commit(patch, &loaded, &applied, (caused, noticed))
            .await
    }

    /// A time of day is stored with the deployment's offset at that moment, which the engine
    /// cannot work out: a patch writing one with another offset is refused here, naming the
    /// right one.
    fn wrong_offsets(&self, patch: &Patch) -> Option<Violations> {
        let wrong = patch
            .mutations
            .as_slice()
            .iter()
            .enumerate()
            .flat_map(|(ordinal, mutation)| {
                mutation.moments().into_iter().filter_map(move |moment| {
                    let When::Moment(wall, offset) = moment else {
                        return None;
                    };
                    let right = self.settings.offset_instead(wall, offset)?;
                    let message = format!(
                        "the time {moment} is written with the deployment's offset at that moment, {}",
                        When::offset_text(right)
                    );
                    Some(violation(
                        ViolationCode::InvalidTime,
                        u32::try_from(ordinal).ok(),
                        message,
                    ))
                })
            })
            .collect();
        Violations::new(wrong).ok()
    }

    /// H5: the answer for a patch the engine rejected. A stale one first checks for the
    /// receipt: the same patch may have committed since the receipt was looked up, and its
    /// own commit is what moved the revision.
    async fn rejected(&self, patch: &Patch, rejection: Rejection) -> Result<Written, WriteError> {
        if matches!(rejection, Rejection::Stale { .. })
            && let Some(answer) = self.answer_from_receipt(patch).await?
        {
            patina_dst::reachable!("service-duplicate-original-committed-meanwhile");
            return answer;
        }
        patina_dst::reachable!("service-stale-patch-completed-with-intervening");
        Err(WriteError::Rejected(rejection))
    }

    /// H5: the answer for a patch id already committed: the receipt for the same content,
    /// a rejection for other content; none for an id never committed.
    pub(crate) async fn answer_from_receipt(
        &self,
        patch: &Patch,
    ) -> Result<Option<Result<Written, WriteError>>, WriteError> {
        let Some(receipt) = self.store.receipt(&patch.id).await? else {
            return Ok(None);
        };
        let same = receipt.content_hash == patch.content_hash() || self.merged_as(patch).await?;
        Ok(Some(if same {
            Ok(Written::AlreadyApplied { receipt })
        } else {
            Err(WriteError::Rejected(Rejection::PatchIdReused {
                patch_id: patch.id.clone(),
            }))
        }))
    }

    /// H5, E6: whether `patch`, a lone identity merge whose id is committed, asks for the
    /// merge that id committed. A merge names the journeys referencing its identities at
    /// their revisions when it was drafted, which a client reads from the index, so a
    /// resubmission after any of them moved carries other content for the same merge; it is
    /// matched by what it merges: the committed patch's
    /// one event merged the same identities into the same survivor.
    async fn merged_as(&self, patch: &Patch) -> Result<bool, WriteError> {
        let [
            Mutation::MergeIdentities {
                survivor, merged, ..
            },
        ] = patch.mutations.as_slice()
        else {
            return Ok(false);
        };
        if patch.target != PatchTarget::Deployment {
            return Ok(false);
        }
        let query = EventQuery {
            log: Some(Domain::Deployment),
            patch: Some(patch.id.clone()),
            size: PageSize::new(2),
            ..EventQuery::default()
        };
        let events = self.store.events(&query).await?.items;
        let [logged] = events.as_slice() else {
            return Ok(false);
        };
        let event = &logged.event;
        // The identities the committed merge removed are exactly the ones asked for, each now
        // an alias of the survivor: a subset or a superset is another merge.
        let removed: BTreeSet<&IdentityKey> = event
            .delta
            .iter()
            .filter_map(|write| match write {
                Write::Remove(cairn_schema::RecordKey::Identity(key)) => Some(key),
                _ => None,
            })
            .collect();
        let asked: BTreeSet<&IdentityKey> = merged.iter().collect();
        let aliased = |alias: &IdentityKey| {
            let put = Record::IdentityAlias {
                alias: alias.clone(),
                identity: survivor.clone(),
            };
            event.delta.contains(&Write::Put(put))
        };
        Ok(event.event_type == EventType::IdentitiesMerged
            && event.subject == Subject::Identity(survivor.clone())
            && removed == asked
            && merged.iter().all(aliased))
    }

    /// Commits what `apply` accepted, rechecking every revision it read, and announces it.
    ///
    /// # Panics
    ///
    /// When the store answers with a receipt other than the change set's.
    async fn commit(
        &self,
        patch: &Patch,
        loaded: &load::Loaded,
        applied: &Applied,
        (caused, noticed): (BTreeMap<JourneyId, Consequences>, Vec<Notice>),
    ) -> Result<Written, WriteError> {
        let commit = Commit {
            target: patch.target.clone(),
            base_revision: patch.base_revision,
            preconditions: loaded.preconditions(patch, applied.records()),
            change_set: applied.change_set().clone(),
        };
        match self.store.commit(commit).await {
            Ok(Committed::Applied {
                receipt,
                deployment,
            }) => {
                assert_eq!(
                    receipt,
                    applied.change_set().receipt,
                    "the store keeps the receipt"
                );
                self.announce(
                    &patch.target,
                    &receipt,
                    deployment,
                    &loaded.records,
                    applied,
                );
                Ok(Written::Applied {
                    receipt,
                    consequences: caused,
                    notices: noticed,
                })
            }
            Ok(Committed::AlreadyApplied(receipt)) => {
                patina_dst::reachable!("service-resubmission-raced-original");
                Ok(Written::AlreadyApplied { receipt })
            }
            Err(CommitError::Rejected(rejection)) => {
                patina_dst::reachable!("service-patch-lost-at-commit");
                Err(WriteError::Rejected(rejection))
            }
            Err(CommitError::Failed(error)) => Err(error.into()),
        }
    }

    /// H6: announces every revision a commit moved: its domain's, or for a patch to a
    /// proposal the proposal's (I6: drafting never moves the destination); each journey and
    /// route a deployment patch deleted (A19); each proposal a domain patch applied; and the
    /// deployment's when an identity create riding in a journey or
    /// route patch moved it, at `deployment`, the revision the store says the commit left
    /// (the engine's records cannot know of another such commit that landed after their
    /// load).
    fn announce(
        &self,
        target: &PatchTarget,
        receipt: &PatchReceipt,
        deployment: Revision,
        before: &Records,
        applied: &Applied,
    ) {
        let domain = receipt.domain.clone();
        if let PatchTarget::Proposal { id, .. } = target {
            self.notifier
                .publish(&RevisionOf::Proposal(id.clone()), receipt.revision);
            return;
        }
        self.notifier
            .publish(&RevisionOf::Domain(domain.clone()), receipt.revision);
        // A19: a journey or route deleted moves past the revision every view holds, so each
        // refetches and finds it gone.
        for write in applied.change_set().writes() {
            if let Write::Put(Record::Deleted { id, .. }) = write {
                let gone = id.domain();
                let held = before.revision(&gone);
                self.notifier
                    .publish(&RevisionOf::Domain(gone), held.next());
            }
        }
        for (id, proposal) in &applied.records().proposals {
            if before.proposals.get(id).map(|held| held.revision) != Some(proposal.revision) {
                self.notifier
                    .publish(&RevisionOf::Proposal(id.clone()), proposal.revision);
            }
        }
        let creates_identities = applied.change_set().writes().any(|write| {
            matches!(
                write,
                Write::Put(Record::Identity(_) | Record::IdentityAlias { .. })
            )
        });
        if domain == Domain::Deployment || !creates_identities {
            return;
        }
        if deployment > before.deployment.revision {
            self.notifier
                .publish(&RevisionOf::Domain(Domain::Deployment), deployment);
        }
    }
}

/// The revision before `revision`: the base a committed patch named, given the revision its
/// receipt says it produced (A17: each patch moves its target by one).
pub(crate) fn before(revision: Revision) -> Revision {
    Revision::try_from(revision.get().saturating_sub(1)).unwrap_or(Revision::NONE)
}

/// A violation the service finds itself, before the engine sees the patch (A15: the same
/// shape as the engine's).
pub(crate) fn violation(code: ViolationCode, mutation: Option<u32>, message: String) -> Violation {
    Violation {
        code,
        at: Location {
            mutation,
            ..Location::default()
        },
        message,
        related: Vec::new(),
        limit: None,
        failures: std::collections::BTreeSet::new(),
        chains: None,
        caused_by: std::collections::BTreeSet::new(),
    }
}

/// Runs an engine call, catching a panic (PRACTICES, Programmer errors panic: the engine
/// works on a private candidate and nothing commits, so the request fails and the host logs
/// it with the domain).
pub(crate) fn engine<T>(domain: &Domain, call: impl FnOnce() -> T) -> Result<T, ServiceError> {
    catch_unwind(AssertUnwindSafe(|| {
        let _inside = crate::observe::EngineCall::enter();
        call()
    }))
    .map_err(|payload| {
        let message = payload
            .downcast_ref::<&str>()
            .map(|text| (*text).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "a panic without a message".to_owned());
        ServiceError::EnginePanic {
            domain: domain.clone(),
            message,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_engine_panic_becomes_an_error_naming_the_domain() {
        let domain = Domain::Journey("j_one".parse().unwrap());
        let caught = engine(&domain, || -> u32 { panic!("an engine assertion failed") });
        assert_eq!(
            caught,
            Err(ServiceError::EnginePanic {
                domain: domain.clone(),
                message: "an engine assertion failed".to_owned(),
            })
        );
        assert_eq!(engine(&domain, || 7), Ok(7));
    }
}
