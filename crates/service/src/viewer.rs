//! Who the caller is in the deployment (H3): the entities holding one of the user's verified
//! sign-in emails, so "mine" needs no separate link step.

use std::collections::BTreeSet;

use cairn_schema::{Deployment, Email, EntityKey, UserId};
use cairn_store::{IdentityRecord, Store};

use crate::{Call, Service, ServiceError};

/// The viewer's entities (H3, E4): those holding one of the user's verified emails. Usually
/// one; none for a user no entity names; two or more when they are duplicates of one person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Viewer {
    /// The user.
    pub user: UserId,
    /// Their entities, which "mine" covers together.
    pub entities: BTreeSet<EntityKey>,
    /// The identities they sign in with (ARCHITECTURE, Auth: users and identities), each
    /// with the verified emails it carried at its last sign-in, in provider and subject
    /// order.
    pub identities: Vec<IdentityRecord>,
}

impl Viewer {
    /// H3: the entities to offer merging, when the user's verified emails match more than one.
    #[must_use]
    pub fn merge_offer(&self) -> Option<&BTreeSet<EntityKey>> {
        (self.entities.len() > 1).then_some(&self.entities)
    }
}

impl<S: Store> Service<S> {
    /// H3: the caller's entities, from the verified emails of every identity they signed in
    /// with, matched against entity emails in the current deployment. Emails are stored
    /// trimmed and lower-cased, so the match is case-insensitive; an email the provider did
    /// not verify is never listed on an identity, so it never matches.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn viewer(&self, call: &Call) -> Result<Viewer, ServiceError> {
        let user = call.actor.user.clone();
        let mut identities = self.store.identities_of(&user).await?;
        identities.sort_by(|a, b| (&a.provider, &a.subject).cmp(&(&b.provider, &b.subject)));
        let deployment = self.deployment().await?;
        Ok(Viewer {
            entities: entities_of(&identities, &deployment),
            user,
            identities,
        })
    }
}

/// The entities holding any verified email of `identities`. Emails are unique across
/// entities, so each email names at most one.
pub(crate) fn entities_of(
    identities: &[IdentityRecord],
    deployment: &Deployment,
) -> BTreeSet<EntityKey> {
    let verified: BTreeSet<&Email> = identities
        .iter()
        .flat_map(|identity| &identity.verified_emails)
        .collect();
    let found: BTreeSet<EntityKey> = deployment
        .entities
        .values()
        .filter(|entity| entity.emails.iter().any(|email| verified.contains(email)))
        .map(|entity| entity.key.clone())
        .collect();
    assert!(found.len() <= verified.len(), "an email names one entity");
    found
}
