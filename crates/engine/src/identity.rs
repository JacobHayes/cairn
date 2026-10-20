//! Identity resolution through aliases (E6, H3): which identity a key names once merges are
//! followed, and which graphs refer to a set of identities. Where a graph names an identity is
//! `cairn_schema::Graph::identity_mentions`, the one rule the stores share.

use std::collections::BTreeSet;

use cairn_schema::{Deployment, Graph, IdentityKey};

/// Whether `graph` names any of `identities` (identities or aliases resolving to one of them):
/// in an answer, a fill, a participation, or an identity-typed condition.
#[must_use]
pub(crate) fn refers_to(
    graph: &Graph,
    deployment: &Deployment,
    identities: &BTreeSet<&IdentityKey>,
) -> bool {
    let mentioned = graph.identity_mentions();
    let compared = graph.condition_identity_mentions();
    let keys = mentioned
        .iter()
        .map(|(_, key)| *key)
        .chain(compared.iter().map(|(_, key)| key));
    keys.into_iter().any(|key| {
        let resolved = resolve(deployment, key).unwrap_or(key);
        identities.contains(resolved)
    })
}

/// E6: the identity a key names, following an alias left by a merge; `None` when it names no
/// identity. Aliases always point at an identity (the deployment stage checks), so one step is
/// the whole walk.
#[must_use]
pub(crate) fn resolve<'a>(
    deployment: &'a Deployment,
    key: &'a IdentityKey,
) -> Option<&'a IdentityKey> {
    if deployment.identities.get(key).is_some() {
        return Some(key);
    }
    let target = deployment.aliases.get(key)?;
    assert!(
        !deployment.aliases.contains_key(target) || deployment.identities.get(target).is_none()
    );
    deployment.identities.get(target).map(|_| target)
}
