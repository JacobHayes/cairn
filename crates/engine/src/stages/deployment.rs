//! The deployment holds (E6, H3): every alias names an identity and is not one itself, so an
//! alias resolves in one step with no cycle, each identity has a name, an email, or both, and
//! each email belongs to one identity; a merge
//! joins two identities' emails and aliases, so each identity is held to its email and alias
//! limits here too. Checked when the patch wrote deployment records. Cost: one pass over
//! identities and aliases.

use std::collections::BTreeMap;

use cairn_schema::{Limit, PatchTarget, Subject, ViolationCode};

use super::Check;
use crate::validate::violation;

/// Runs the deployment checks.
pub(super) fn check(check: &mut Check<'_, '_>) {
    let session = check.session;
    let wrote = matches!(session.patch.target, PatchTarget::Deployment)
        || !session.created_identities.is_empty();
    if !wrote {
        return;
    }
    let deployment = &session.candidate.deployment;
    for (alias, target) in &deployment.aliases {
        if deployment.identities.get(alias).is_some() || deployment.identities.get(target).is_none()
        {
            let mut found = violation(
                ViolationCode::AliasCycle,
                "an alias resolves to one existing identity and is not an identity itself (E6)",
            );
            found.at.subject = Some(Subject::Identity(alias.clone()));
            found.related.push(Subject::Identity(target.clone()));
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
    identity_limits(check);
    let mut holders = BTreeMap::new();
    for identity in deployment.identities.values() {
        if identity.name.is_none() && identity.emails.is_empty() {
            let mut found = violation(
                ViolationCode::IdentityEmpty,
                "an identity has a name, an email, or both (H3)",
            );
            found.at.subject = Some(Subject::Identity(identity.key.clone()));
            check.violations.push(found);
        }
        for email in &identity.emails {
            if let Some(first) = holders.insert(email, &identity.key) {
                let mut found = violation(
                    ViolationCode::EmailTaken,
                    format!("{} is held by two identities (H3)", email.as_str()),
                );
                found.at.subject = Some(Subject::Identity(identity.key.clone()));
                found.related.push(Subject::Identity(first.clone()));
                check.violations.push(found);
            }
        }
    }
}

/// PRACTICES, Explicit limits: each identity holds at most `email_count_per_identity_max` emails
/// and is the target of at most `alias_count_per_identity_max` aliases.
fn identity_limits(check: &mut Check<'_, '_>) {
    let deployment = &check.session.candidate.deployment;
    let emails = deployment.identities.values().filter_map(|identity| {
        Limit::EmailCountPerIdentity
            .check(identity.emails.len())
            .err()
            .map(|exceeded| (&identity.key, exceeded))
    });
    let aliases = deployment.alias_counts_exceeded();
    for (key, exceeded) in emails.chain(aliases) {
        let mut found = violation(
            ViolationCode::LimitExceeded,
            format!(
                "the identity has {}, past {}",
                exceeded.count,
                exceeded.limit.name()
            ),
        );
        found.at.subject = Some(Subject::Identity(key.clone()));
        found.limit = Some(exceeded.limit);
        check.violations.push(found);
    }
}
