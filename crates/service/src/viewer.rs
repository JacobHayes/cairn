//! Who the caller is in the deployment (H3): the identities holding one of the user's verified
//! sign-in emails, so "mine" needs no separate link step.

use std::collections::BTreeSet;

use cairn_schema::{Deployment, Email, IdentityKey, Title, UserId};
use cairn_store::LoginRecord;

use crate::{Call, Service, ServiceError};

/// The viewer's identities (H3, E4): those holding one of the user's verified emails. Usually
/// one; none for a user no identity names; two or more when they are duplicates of one person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Viewer {
    /// The user.
    pub user: UserId,
    /// Their identities, which "mine" covers together.
    pub identities: BTreeSet<IdentityKey>,
    /// The logins they sign in with (ARCHITECTURE, Auth: users and logins), each
    /// with the verified emails it carried at its last sign-in, in provider and subject
    /// order.
    pub logins: Vec<LoginRecord>,
}

/// A user and the name people see for them (H3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserName {
    /// The user.
    pub user: UserId,
    /// Their name: the identity their verified emails name, else the name their sign-in gave.
    pub name: Title,
}

impl Viewer {
    /// H3: the identities to offer merging, when the user's verified emails match more than one.
    #[must_use]
    pub fn merge_offer(&self) -> Option<&BTreeSet<IdentityKey>> {
        (self.identities.len() > 1).then_some(&self.identities)
    }
}

impl Service {
    /// H3: the caller's identities, from the verified emails of every login they signed in
    /// with, matched against identity emails in the current deployment. Emails are stored
    /// trimmed and lower-cased, so the match is case-insensitive; an email the provider did
    /// not verify is never listed on a login, so it never matches.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn viewer(&self, call: &Call) -> Result<Viewer, ServiceError> {
        let user = call.actor.user.clone();
        let mut logins = self.store.logins_of(&user).await?;
        logins.sort_by(|a, b| (&a.provider, &a.subject).cmp(&(&b.provider, &b.subject)));
        let deployment = self.deployment().await?;
        Ok(Viewer {
            identities: identities_of(&logins, &deployment),
            user,
            logins,
        })
    }

    /// H3: the names of `users`, in the order asked, leaving out users that do not exist. A
    /// user whose verified emails name an identity goes by that identity's name (the person as
    /// the deployment curates them; the first by key when the emails name several, which
    /// are duplicates of one person), else by the name their sign-in gave. Nothing here
    /// ever answers an id as a name.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn user_names(&self, users: &[UserId]) -> Result<Vec<UserName>, ServiceError> {
        let deployment = self.deployment().await?;
        let mut names = Vec::with_capacity(users.len());
        for user in users {
            let Some(record) = self.store.user(user).await? else {
                continue;
            };
            let logins = self.store.logins_of(user).await?;
            let identity = identities_of(&logins, &deployment)
                .into_iter()
                .find_map(|key| {
                    deployment
                        .identities
                        .values()
                        .find(|identity| identity.key == key)
                        .map(|identity| identity.name.clone())
                });
            names.push(UserName {
                user: user.clone(),
                name: identity.unwrap_or(record.name),
            });
        }
        Ok(names)
    }
}

/// The identities holding any verified email of `logins`. Emails are unique across
/// identities, so each email names at most one.
pub(crate) fn identities_of(
    logins: &[LoginRecord],
    deployment: &Deployment,
) -> BTreeSet<IdentityKey> {
    let verified: BTreeSet<&Email> = logins
        .iter()
        .flat_map(|login| &login.verified_emails)
        .collect();
    let found: BTreeSet<IdentityKey> = deployment
        .identities
        .values()
        .filter(|identity| identity.emails.iter().any(|email| verified.contains(email)))
        .map(|identity| identity.key.clone())
        .collect();
    assert!(found.len() <= verified.len(), "an email names one identity");
    found
}
