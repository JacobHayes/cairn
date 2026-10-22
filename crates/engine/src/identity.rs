//! Identity resolution through aliases (E6, H3): which identity a key names once merges are
//! followed, and which graphs and journeys refer to a set of identities. Where they name an identity is
//! `cairn_schema::Journey::identity_mentions` and `Graph::identity_mentions`, the one rule the
//! stores share.

use std::collections::BTreeSet;

use cairn_schema::{Deployment, IdentityKey};

/// Whether any of `keys` (identities or aliases) resolves to one of `identities`. Callers pass
/// what a journey or a route's graph refers to (`Journey::identity_references`,
/// `Graph::identity_references`).
#[must_use]
pub(crate) fn refers_to<'a>(
    keys: impl IntoIterator<Item = &'a IdentityKey>,
    deployment: &Deployment,
    identities: &BTreeSet<&IdentityKey>,
) -> bool {
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
