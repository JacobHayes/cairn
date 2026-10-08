//! Entities (E6, H3): create in any patch, edit and merge in the deployment's.

use std::collections::BTreeMap;

use cairn_schema::{
    Domain, Entity, EntityKey, JourneyId, Limit, Mutation, Record, RecordKey, Revision,
    RevisionConflict, RevisionOf, Subject, ViolationCode, Write,
};

use super::Session;
use crate::entity;

/// Applies an entity mutation.
pub(super) fn apply(session: &mut Session<'_>, mutation: &Mutation) -> Vec<Write> {
    match mutation {
        Mutation::CreateEntity { entity } => create(session, entity),
        Mutation::EditEntity { entity } => {
            if session
                .candidate
                .deployment
                .entities
                .get(&entity.key)
                .is_none()
            {
                reject_entity(
                    session,
                    ViolationCode::EntityUnresolved,
                    &entity.key,
                    "no such entity to edit",
                );
                return Vec::new();
            }
            vec![Write::Put(Record::Entity(entity.clone()))]
        }
        Mutation::MergeEntities {
            survivor,
            merged,
            journeys,
        } => merge(session, survivor, merged, journeys),
        other => unreachable!("{other:?} is dispatched elsewhere"),
    }
}

fn reject_entity(session: &mut Session<'_>, code: ViolationCode, key: &EntityKey, message: &str) {
    session.reject(code, None, message);
    if let Some(found) = session.violations.last_mut() {
        found.at.subject = Some(Subject::Entity(key.clone()));
    }
}

/// E6: a create needs no deployment revision, so a journey patch can create a person and
/// answer with them at once; a key already an entity or an alias is rejected, and so is an
/// entity past `entity_count_per_deployment_max`.
fn create(session: &mut Session<'_>, entity: &Entity) -> Vec<Write> {
    let deployment = &session.candidate.deployment;
    if deployment.entities.get(&entity.key).is_some()
        || deployment.aliases.contains_key(&entity.key)
    {
        reject_entity(
            session,
            ViolationCode::EntityKeyTaken,
            &entity.key,
            "the key is already an entity or an alias (E6)",
        );
        return Vec::new();
    }
    if Limit::EntityCountPerDeployment
        .check(deployment.entities.len() + 1)
        .is_err()
    {
        reject_entity(
            session,
            ViolationCode::LimitExceeded,
            &entity.key,
            "past entity_count_per_deployment_max",
        );
        if let Some(found) = session.violations.last_mut() {
            found.limit = Some(Limit::EntityCountPerDeployment);
        }
        return Vec::new();
    }
    session.created_entities.insert(entity.key.clone());
    assert!(session.created_entities.contains(&entity.key));
    vec![Write::Put(Record::Entity(entity.clone()))]
}

/// E6, H3: the merged entity's key becomes an alias of the survivor, which takes its emails;
/// aliases of the merged entity follow it, so every alias stays one step from an entity. The
/// journeys referencing either must be exactly those the merge names, at their revisions,
/// and each is validated against the merged deployment.
fn merge(
    session: &mut Session<'_>,
    survivor: &EntityKey,
    merged: &EntityKey,
    journeys: &BTreeMap<JourneyId, Revision>,
) -> Vec<Write> {
    let deployment = &session.candidate.deployment;
    let (Some(kept), Some(gone)) = (
        deployment.entities.get(survivor),
        deployment.entities.get(merged),
    ) else {
        reject_entity(
            session,
            ViolationCode::EntityUnresolved,
            merged,
            "a merge joins two existing entities (E6)",
        );
        return Vec::new();
    };
    if survivor == merged {
        reject_entity(
            session,
            ViolationCode::AliasCycle,
            merged,
            "an entity cannot merge into itself (E6)",
        );
        return Vec::new();
    }
    assert_ne!(survivor, merged);
    let mut joined = kept.clone();
    joined.emails.extend(gone.emails.iter().cloned());
    // Every record an event carries reads back, so a merge that would put an entity past its
    // email limit, or name more journeys than a merge may, is refused here rather than only
    // by the final check, which a later edit in the patch could pass (PRACTICES, Explicit
    // limits).
    let counts = [
        (Limit::EmailCountPerEntity, joined.emails.len()),
        (Limit::JourneyCountPerMerge, journeys.len()),
    ];
    if let Some(exceeded) = counts
        .into_iter()
        .find_map(|(limit, count)| limit.check(count).err())
    {
        let message = format!("{} past {}", exceeded.count, exceeded.limit.name());
        reject_entity(session, ViolationCode::LimitExceeded, survivor, &message);
        if let Some(found) = session.violations.last_mut() {
            found.limit = Some(exceeded.limit);
        }
        return Vec::new();
    }
    let mut writes = vec![
        Write::Remove(RecordKey::Entity(merged.clone())),
        Write::Put(Record::Entity(joined)),
        Write::Put(Record::EntityAlias {
            alias: merged.clone(),
            entity: survivor.clone(),
        }),
    ];
    for (alias, target) in &deployment.aliases {
        if target == merged {
            writes.push(Write::Put(Record::EntityAlias {
                alias: alias.clone(),
                entity: survivor.clone(),
            }));
        }
    }
    assert!(
        joined_holds_both(&writes, gone),
        "the survivor takes the merged entity's emails (H3)"
    );
    check_journeys(session, [survivor, merged], journeys);
    writes
}

/// The journeys loaded for a merge that reference either entity must be the ones it names,
/// at the revisions it names (E6, H5); each is then validated with every merge of the patch
/// applied.
fn check_journeys(
    session: &mut Session<'_>,
    entities: [&EntityKey; 2],
    named: &BTreeMap<JourneyId, Revision>,
) {
    let deployment = &session.candidate.deployment;
    let mut referencing = Vec::new();
    for (id, journey) in &session.candidate.journeys {
        let mentions = entity::mentions(&journey.graph);
        let conditions = entity::condition_mentions(&journey.graph);
        let keys = mentions
            .iter()
            .map(|(_, key)| *key)
            .chain(conditions.iter().map(|(_, key)| key));
        let refers = keys.into_iter().any(|key| {
            let resolved = entity::resolve(deployment, key).unwrap_or(key);
            entities.contains(&resolved)
        });
        if refers {
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

fn joined_holds_both(writes: &[Write], gone: &Entity) -> bool {
    writes.iter().any(|write| match write {
        Write::Put(Record::Entity(joined)) => joined.emails.is_superset(&gone.emails),
        Write::Put(_) | Write::Remove(_) | Write::CopyGraph { .. } => false,
    })
}
