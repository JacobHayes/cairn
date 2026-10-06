//! The deployment holds (E6, H3): every alias names an entity and is not one itself, so an
//! alias resolves in one step with no cycle, and each email belongs to one entity. Checked
//! when the patch wrote deployment records. Cost: one pass over entities and aliases.

use std::collections::BTreeMap;

use cairn_schema::{Limit, PatchTarget, Subject, ViolationCode};

use super::Check;
use crate::validate::violation;

/// Runs the deployment checks.
pub(super) fn check(check: &mut Check<'_, '_>) {
    let session = check.session;
    let wrote = matches!(session.patch.target, PatchTarget::Deployment)
        || !session.created_entities.is_empty();
    if !wrote {
        return;
    }
    let deployment = &session.candidate.deployment;
    for (alias, target) in &deployment.aliases {
        if deployment.entities.get(alias).is_some() || deployment.entities.get(target).is_none() {
            let mut found = violation(
                ViolationCode::AliasCycle,
                "an alias resolves to one existing entity and is not an entity itself (E6)",
            );
            found.at.subject = Some(Subject::Entity(alias.clone()));
            found.related.push(Subject::Entity(target.clone()));
            check.violations.push(found);
        }
    }
    // PRACTICES, Explicit limits: the deployment record, like a graph, is at most 16 MiB.
    let bytes = match cairn_schema::to_json(deployment) {
        Ok(json) => json.len(),
        Err(error) => panic!("a deployment always serializes: {error}"),
    };
    if Limit::GraphBytes.check(bytes).is_err() {
        let mut found = violation(
            ViolationCode::LimitExceeded,
            format!("the deployment serializes to {bytes} bytes, past graph_bytes_max"),
        );
        found.at.subject = Some(Subject::Deployment);
        found.limit = Some(Limit::GraphBytes);
        check.violations.push(found);
    }
    let mut holders = BTreeMap::new();
    for entity in deployment.entities.values() {
        for email in &entity.emails {
            if let Some(first) = holders.insert(email, &entity.key) {
                let mut found = violation(
                    ViolationCode::EmailTaken,
                    format!("{} is held by two entities (H3)", email.as_str()),
                );
                found.at.subject = Some(Subject::Entity(entity.key.clone()));
                found.related.push(Subject::Entity(first.clone()));
                check.violations.push(found);
            }
        }
    }
}
