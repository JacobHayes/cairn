//! The deployment's mutations: identities (E6, H3), created in any patch and edited, merged
//! and deleted in the deployment's, and the hard deletion of journeys and routes (A19).

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    DeletedId, Domain, GraphId, Identity, IdentityKey, IdentitySet, JourneyId, Limit, Mutation,
    Record, RecordKey, Revision, RevisionConflict, RevisionOf, Subject, ViolationCode, Write,
    limits::CHAIN_COUNT_PER_REJECTION_MAX,
};

use super::Session;
use crate::identity;

/// Applies an identity mutation.
pub(super) fn apply(session: &mut Session<'_>, mutation: &Mutation) -> Vec<Write> {
    match mutation {
        Mutation::CreateIdentity { identity } => create(session, identity),
        Mutation::EditIdentity { identity } => {
            if session
                .candidate
                .deployment
                .identities
                .get(&identity.key)
                .is_none()
            {
                reject_identity(
                    session,
                    ViolationCode::IdentityUnresolved,
                    &identity.key,
                    "no such identity to edit",
                );
                return Vec::new();
            }
            vec![Write::Put(Record::Identity(identity.clone()))]
        }
        Mutation::MergeIdentities {
            survivor,
            merged,
            journeys,
        } => merge(session, survivor, merged, journeys),
        Mutation::DeleteIdentity { identity } => delete(session, identity),
        Mutation::DeleteJourney { journey, revision } => {
            hard_delete(session, &DeletedId::Journey(journey.clone()), *revision)
        }
        Mutation::DeleteRoute { route, revision } => {
            hard_delete(session, &DeletedId::Route(route.clone()), *revision)
        }
        other => unreachable!("{other:?} is dispatched elsewhere"),
    }
}

fn reject_identity(
    session: &mut Session<'_>,
    code: ViolationCode,
    key: &IdentityKey,
    message: &str,
) {
    session.reject(code, None, message);
    if let Some(found) = session.violations.last_mut() {
        found.at.subject = Some(Subject::Identity(key.clone()));
    }
}

/// E6: a create needs no deployment revision, so a journey patch can create a person and
/// answer with them at once; a key already an identity or an alias is rejected, and so is an
/// identity past `identity_count_per_deployment_max`.
fn create(session: &mut Session<'_>, identity: &Identity) -> Vec<Write> {
    let deployment = &session.candidate.deployment;
    if deployment.identities.get(&identity.key).is_some()
        || deployment.aliases.contains_key(&identity.key)
    {
        reject_identity(
            session,
            ViolationCode::IdentityKeyTaken,
            &identity.key,
            "the key is already an identity or an alias",
        );
        return Vec::new();
    }
    if Limit::IdentityCountPerDeployment
        .check(deployment.identities.len() + 1)
        .is_err()
    {
        reject_identity(
            session,
            ViolationCode::LimitExceeded,
            &identity.key,
            "past identity_count_per_deployment_max",
        );
        if let Some(found) = session.violations.last_mut() {
            found.limit = Some(Limit::IdentityCountPerDeployment);
        }
        return Vec::new();
    }
    session.created_identities.insert(identity.key.clone());
    assert!(session.created_identities.contains(&identity.key));
    vec![Write::Put(Record::Identity(identity.clone()))]
}

/// E6, H3: each merged identity's key becomes an alias of the survivor, which takes its emails;
/// aliases of a merged identity follow it, so every alias stays one step from an identity. The
/// journeys referencing the survivor or any merged identity must be exactly those the merge
/// names, at their revisions, and each is validated against the merged deployment: one group
/// is one snapshot, so a merge of several is not a chain of pairs.
fn merge(
    session: &mut Session<'_>,
    survivor: &IdentityKey,
    merged: &IdentitySet,
    journeys: &BTreeMap<JourneyId, Revision>,
) -> Vec<Write> {
    if merge_refused(session, survivor, merged) {
        return Vec::new();
    }
    let deployment = &session.candidate.deployment;
    let Some(kept) = deployment.identities.get(survivor) else {
        unreachable!("the survivor was found above")
    };
    let mut joined = kept.clone();
    for key in merged.iter() {
        let Some(gone) = deployment.identities.get(key) else {
            unreachable!("every merged identity was found above")
        };
        joined.emails.extend(gone.emails.iter().cloned());
    }
    // Every record an event carries reads back, so a merge that would put an identity past its
    // email limit, or name more journeys than a merge may, is refused here rather than only
    // by the final check, which a later edit in the patch could pass (PRACTICES, Explicit
    // limits).
    let counts = [
        (Limit::EmailCountPerIdentity, joined.emails.len()),
        (Limit::JourneyCountPerMerge, journeys.len()),
    ];
    if let Some(exceeded) = counts
        .into_iter()
        .find_map(|(limit, count)| limit.check(count).err())
    {
        let message = format!("{} past {}", exceeded.count, exceeded.limit.name());
        reject_identity(session, ViolationCode::LimitExceeded, survivor, &message);
        if let Some(found) = session.violations.last_mut() {
            found.limit = Some(exceeded.limit);
        }
        return Vec::new();
    }
    let mut writes: Vec<Write> = merged
        .iter()
        .map(|key| Write::Remove(RecordKey::Identity(key.clone())))
        .collect();
    writes.push(Write::Put(Record::Identity(joined)));
    writes.extend(merged.iter().map(|key| {
        Write::Put(Record::IdentityAlias {
            alias: key.clone(),
            identity: survivor.clone(),
        })
    }));
    for (alias, target) in &deployment.aliases {
        if merged.as_set().contains(target) {
            writes.push(Write::Put(Record::IdentityAlias {
                alias: alias.clone(),
                identity: survivor.clone(),
            }));
        }
    }
    let affected: BTreeSet<&IdentityKey> = std::iter::once(survivor).chain(merged.iter()).collect();
    check_journeys(session, &affected, journeys);
    writes
}

/// E6: whether a merge's identities are unfit to merge, with the violations that say why: none
/// to merge, the survivor among them, or one that does not exist.
fn merge_refused(session: &mut Session<'_>, survivor: &IdentityKey, merged: &IdentitySet) -> bool {
    let deployment = &session.candidate.deployment;
    if merged.is_empty() {
        reject_identity(
            session,
            ViolationCode::IdentityUnresolved,
            survivor,
            "a merge names at least one identity to merge into the survivor",
        );
        return true;
    }
    if merged.as_set().contains(survivor) {
        reject_identity(
            session,
            ViolationCode::AliasCycle,
            survivor,
            "an identity cannot merge into itself",
        );
        return true;
    }
    let missing: Vec<IdentityKey> = std::iter::once(survivor)
        .chain(merged.iter())
        .filter(|key| deployment.identities.get(key).is_none())
        .cloned()
        .collect();
    if !missing.is_empty() {
        for key in &missing {
            reject_identity(
                session,
                ViolationCode::IdentityUnresolved,
                key,
                "a merge joins existing identities",
            );
        }
        return true;
    }
    false
}

/// E6: an identity nothing refers to goes with its emails and every alias resolving to it. A
/// journey, route draft, or route version naming it or one of its aliases keeps it: the
/// graphs loaded for the patch (any one is enough to refuse) are listed, and the commit
/// rechecks that none refers to it.
fn delete(session: &mut Session<'_>, identity: &IdentityKey) -> Vec<Write> {
    let deployment = &session.candidate.deployment;
    if deployment.identities.get(identity).is_none() {
        reject_identity(
            session,
            ViolationCode::IdentityUnresolved,
            identity,
            "no such identity to delete",
        );
        return Vec::new();
    }
    let aliases: Vec<IdentityKey> = deployment
        .aliases
        .iter()
        .filter(|(_, target)| *target == identity)
        .map(|(alias, _)| alias.clone())
        .collect();
    let named: BTreeSet<&IdentityKey> = std::iter::once(identity).chain(&aliases).collect();
    let referring = referring_graphs(session, &named);
    if !referring.is_empty() {
        // The rejection lists what refers to the identity, up to the limit, and says how many
        // more there are: those beyond the loaded graphs are counted by the host.
        let listed = referring.len().min(CHAIN_COUNT_PER_REJECTION_MAX as usize);
        let unlisted = u32::try_from(referring.len() - listed).unwrap_or(u32::MAX)
            + session
                .candidate
                .unlisted_references
                .get(&Subject::Identity(identity.clone()))
                .copied()
                .unwrap_or_default();
        let more = if unlisted == 0 {
            String::new()
        } else {
            format!(", and {unlisted} more not listed")
        };
        reject_identity(
            session,
            ViolationCode::IdentityInUse,
            identity,
            &format!(
                "journeys or routes still refer to the identity{more}; merge it into another instead"
            ),
        );
        if let Some(found) = session.violations.last_mut() {
            found.related.extend(referring.into_iter().take(listed));
        }
        return Vec::new();
    }
    let mut writes: Vec<Write> = aliases
        .into_iter()
        .map(|alias| Write::Remove(RecordKey::IdentityAlias(alias)))
        .collect();
    writes.push(Write::Remove(RecordKey::Identity(identity.clone())));
    writes
}

/// The journeys, and the routes with a draft or version, among the records loaded that name
/// any of `identities`.
fn referring_graphs(session: &Session<'_>, identities: &BTreeSet<&IdentityKey>) -> Vec<Subject> {
    let candidate = &session.candidate;
    let deployment = &candidate.deployment;
    let mut found = Vec::new();
    let graphs = candidate
        .journeys
        .keys()
        .map(|id| GraphId::Journey(id.clone()))
        .chain(
            candidate
                .routes
                .iter()
                .filter(|(_, route)| route.draft.is_some())
                .map(|(id, _)| GraphId::RouteDraft(id.clone())),
        )
        .chain(
            candidate
                .versions
                .keys()
                .map(|lineage| GraphId::RouteVersion {
                    route: lineage.route.clone(),
                    version: lineage.version,
                }),
        );
    for id in graphs {
        let Some(graph) = candidate.graph(&id) else {
            continue;
        };
        if !identity::refers_to(graph, deployment, identities) {
            continue;
        }
        let subject = match id {
            GraphId::Journey(journey) => Subject::Journey(journey),
            GraphId::RouteDraft(route) | GraphId::RouteVersion { route, .. } => {
                Subject::Route(route)
            }
        };
        if !found.contains(&subject) {
            found.push(subject);
        }
    }
    found
}

/// The journeys loaded for a merge that reference any of the identities must be the ones it
/// names, at the revisions it names (E6, H5); each is then validated with every merge of the
/// patch applied.
fn check_journeys(
    session: &mut Session<'_>,
    identities: &BTreeSet<&IdentityKey>,
    named: &BTreeMap<JourneyId, Revision>,
) {
    let deployment = &session.candidate.deployment;
    let mut referencing = Vec::new();
    for (id, journey) in &session.candidate.journeys {
        if identity::refers_to(&journey.graph, deployment, identities) {
            referencing.push((id.clone(), journey.revision));
        }
    }
    for (id, current) in &referencing {
        let expected = named.get(id).copied().unwrap_or(Revision::NONE);
        if expected != *current {
            session.conflicts.push(RevisionConflict {
                of: RevisionOf::Domain(Domain::Journey(id.clone())),
                expected,
                current: *current,
            });
        }
    }
    for (id, expected) in named {
        if !referencing.iter().any(|(found, _)| found == id) {
            let current = session
                .candidate
                .journeys
                .get(id)
                .map_or(Revision::NONE, |journey| journey.revision);
            session.conflicts.push(RevisionConflict {
                of: RevisionOf::Domain(Domain::Journey(id.clone())),
                expected: *expected,
                current,
            });
        }
    }
    // A later merge in the patch adds its journeys; it never drops an earlier merge's (E6).
    session
        .merge_checked
        .extend(referencing.into_iter().map(|(id, _)| id));
}

// PRD: A19.
/// A hard delete of a journey or a route: the domain goes whole, its id is recorded so it is
/// never used again, and the deployment log keeps the event. The domain must be at the
/// revision the deletion names, as the host loaded it; one that moved or is gone makes the
/// patch stale, every such one listed, and the commit checks the revisions again. A route still
/// in use is refused once the whole patch has applied (stages), so a patch can delete a route
/// with the journeys on it in any order.
fn hard_delete(session: &mut Session<'_>, id: &DeletedId, named: Revision) -> Vec<Write> {
    let domain = id.domain();
    if !session.deleted.insert(id.clone()) {
        session.reject(
            ViolationCode::TargetMissing,
            None,
            format!("{domain} is deleted earlier in this patch"),
        );
        if let Some(found) = session.violations.last_mut() {
            found.at.subject = Some(deleted_subject(id));
        }
        return Vec::new();
    }
    let current = session.candidate.revision(&domain);
    if current == Revision::NONE && named == Revision::NONE {
        session.reject(
            ViolationCode::TargetMissing,
            None,
            format!("{domain} does not exist"),
        );
        if let Some(found) = session.violations.last_mut() {
            found.at.subject = Some(deleted_subject(id));
        }
        return Vec::new();
    }
    if current != named {
        session.conflicts.push(RevisionConflict {
            of: RevisionOf::Domain(domain),
            expected: named,
            current,
        });
        return Vec::new();
    }
    vec![
        Write::Remove(RecordKey::Domain(domain)),
        Write::Put(Record::Deleted {
            id: id.clone(),
            deleted_at: session.inputs.at,
        }),
    ]
}

/// The subject a deletion's violation is about.
pub(crate) fn deleted_subject(id: &DeletedId) -> Subject {
    match id {
        DeletedId::Journey(journey) => Subject::Journey(journey.clone()),
        DeletedId::Route(route) => Subject::Route(route.clone()),
    }
}
