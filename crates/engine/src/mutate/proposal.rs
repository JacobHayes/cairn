//! Proposals (I6, H5): created, edited, and discarded through their own revisions without
//! touching their destination, and applied in a patch to the destination, where their
//! mutations run in order exactly as if the patch held them and every check applies.

use cairn_schema::{
    Mutation, PatchTarget, Proposal, ProposalStatus, Record, RevisionConflict, RevisionOf, Subject,
    ViolationCode, Write,
};

use super::Session;

/// Applies a proposal create, edit, or discard to the proposal the patch targets.
pub(super) fn apply(session: &mut Session<'_>, mutation: &Mutation) -> Vec<Write> {
    let PatchTarget::Proposal { id, destination } = &session.patch.target else {
        unreachable!("proposal mutations are admitted only in patches to a proposal")
    };
    let existing = session.candidate.proposals.get(id).cloned();
    let revision = session.patch.base_revision.next();
    let inputs = session.inputs;
    let next = match (mutation, existing) {
        (Mutation::CreateProposal { proposal }, None) => Proposal {
            id: id.clone(),
            destination: destination.clone(),
            revision,
            status: ProposalStatus::Open,
            draft: proposal.clone(),
            proposing_agent: inputs.actor.agent.clone(),
            created_by: inputs.actor.user.clone(),
            created_at: inputs.at,
        },
        (Mutation::CreateProposal { .. }, Some(_)) => {
            reject(session, ViolationCode::TargetExists, "the proposal exists");
            return Vec::new();
        }
        (Mutation::EditProposal { .. } | Mutation::DiscardProposal, None) => {
            reject(session, ViolationCode::TargetMissing, "no such proposal");
            return Vec::new();
        }
        (Mutation::EditProposal { .. } | Mutation::DiscardProposal, Some(found))
            if found.destination != *destination =>
        {
            reject(
                session,
                ViolationCode::MutationNotForTarget,
                "the proposal belongs to another destination",
            );
            return Vec::new();
        }
        (Mutation::EditProposal { .. } | Mutation::DiscardProposal, Some(found))
            if found.status != ProposalStatus::Open =>
        {
            reject(
                session,
                ViolationCode::ProposalNotOpen,
                "the proposal was applied or discarded",
            );
            return Vec::new();
        }
        (Mutation::EditProposal { proposal }, Some(found)) => Proposal {
            revision,
            draft: proposal.clone(),
            ..found
        },
        (Mutation::DiscardProposal, Some(found)) => Proposal {
            revision,
            status: ProposalStatus::Discarded,
            ..found
        },
        (other, _) => unreachable!("{other:?} does not edit a proposal"),
    };
    assert_eq!(
        next.revision, revision,
        "a proposal patch advances its revision by one"
    );
    assert_eq!(next.id, *id);
    vec![Write::Put(Record::Proposal(next))]
}

fn reject(session: &mut Session<'_>, code: ViolationCode, message: &str) {
    session.reject(code, None, message);
    if let (Some(found), PatchTarget::Proposal { id, .. }) =
        (session.violations.last_mut(), &session.patch.target)
    {
        found.at.subject = Some(Subject::Proposal(id.clone()));
    }
}

/// I6, H5: applying a proposal checks the revision its reviewer saw and the destination
/// revision it was drafted against, runs its mutations in order (each applied to the
/// candidate and appended to `delta`), and marks it applied, all in this one event.
pub(super) fn apply_to_destination(
    session: &mut Session<'_>,
    mutation: &Mutation,
    delta: &mut Vec<Write>,
) -> Vec<Write> {
    let Mutation::ApplyProposal {
        proposal: id,
        reviewed_revision,
    } = mutation
    else {
        unreachable!("dispatched for proposal application only")
    };
    let Some(found) = session.candidate.proposals.get(id).cloned() else {
        session.reject(
            ViolationCode::TargetMissing,
            None,
            format!("no proposal {id}"),
        );
        return Vec::new();
    };
    let destination = session.domain();
    let problem = if found.destination != destination {
        Some((
            ViolationCode::MutationNotForTarget,
            "the proposal belongs to another destination",
        ))
    } else if found.status != ProposalStatus::Open {
        Some((
            ViolationCode::ProposalNotOpen,
            "the proposal was applied or discarded",
        ))
    } else {
        None
    };
    if let Some((code, message)) = problem {
        reject_about(session, code, id, message);
        return Vec::new();
    }
    stale_unless(
        session,
        RevisionOf::Proposal(id.clone()),
        *reviewed_revision,
        found.revision,
    );
    let base = session.patch.base_revision;
    stale_unless(
        session,
        RevisionOf::Domain(destination.clone()),
        found.draft.destination_revision,
        base,
    );
    assert_eq!(found.status, ProposalStatus::Open);
    let archived = session
        .journey()
        .is_some_and(|journey| journey.header.status == cairn_schema::JourneyStatus::Archived);
    if archived && found.draft.mutations.is_empty() {
        // Each of a proposal's mutations is held to the archived ban; an empty one would
        // otherwise write to an archived journey (B11).
        reject_about(
            session,
            ViolationCode::ArchivedJourney,
            id,
            "an archived journey accepts only un-archiving or hard deletion (B11, A19)",
        );
        return Vec::new();
    }
    let existed = exists(session, &destination);
    if !run_mutations(session, &found, existed, delta) {
        // The patch is rejected; the proposal stays open.
        return Vec::new();
    }
    let applied = Proposal {
        revision: found.revision.next(),
        status: ProposalStatus::Applied,
        ..found
    };
    vec![Write::Put(Record::Proposal(applied))]
}

/// Runs a proposal's mutations in order, each applied to the candidate and appended to
/// `delta`; true when every one applied and the destination exists afterwards.
fn run_mutations(
    session: &mut Session<'_>,
    found: &Proposal,
    existed: bool,
    delta: &mut Vec<Write>,
) -> bool {
    let found_before = session.violations.len();
    for (position, inner) in found.draft.mutations.as_slice().iter().enumerate() {
        session.inner = Some(u32::try_from(position).unwrap_or(u32::MAX));
        super::apply(session, inner, delta);
    }
    session.inner = None;
    // A proposal for a new destination must create it; one for an existing destination may
    // delete it (A19).
    let created = existed || exists(session, &session.domain());
    if !created && session.violations.len() == found_before {
        session.reject(
            ViolationCode::TargetMissing,
            None,
            "the proposal does not create its destination",
        );
    }
    assert!(created || session.violations.len() > found_before);
    session.violations.len() == found_before
}

fn exists(session: &Session<'_>, domain: &cairn_schema::Domain) -> bool {
    match domain {
        cairn_schema::Domain::Journey(id) => session.candidate.journeys.contains_key(id),
        cairn_schema::Domain::Route(id) => session.candidate.routes.contains_key(id),
        cairn_schema::Domain::Deployment => true,
    }
}

fn reject_about(
    session: &mut Session<'_>,
    code: ViolationCode,
    id: &cairn_schema::ProposalId,
    message: &str,
) {
    session.reject(code, None, message);
    if let Some(found) = session.violations.last_mut() {
        found.at.subject = Some(Subject::Proposal(id.clone()));
    }
}

/// H5: a revision the reviewer or drafter saw that has moved on makes the patch stale.
fn stale_unless(
    session: &mut Session<'_>,
    of: RevisionOf,
    expected: cairn_schema::Revision,
    current: cairn_schema::Revision,
) {
    if expected != current {
        session.conflicts.push(RevisionConflict {
            of,
            expected,
            current,
        });
    }
}
