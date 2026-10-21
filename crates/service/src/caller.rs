//! Who the caller is in the deployment (H3): the identities holding one of the user's
//! verified sign-in emails, so "mine" needs no separate link step, and the arrival that
//! gives a user their identity the first time they are seen.

use std::collections::BTreeSet;

use cairn_schema::limits::PAGE_ITEM_COUNT_MAX;
use cairn_schema::{
    Actor, Deployment, Email, Identity, IdentityKey, KeyAllocator, Mutation, Mutations, Patch,
    PatchId, PatchTarget, Revision, Title, UserId, mint,
};
use cairn_store::LoginRecord;

use crate::{Call, DomainPatch, ReadError, Service, ServiceError, WriteError};

/// How many times an arrival tries to create the identity before giving up: a lost race
/// costs one more try, and nothing in a sound deployment costs three.
const ARRIVAL_ATTEMPT_COUNT_MAX: usize = 3;

/// The caller (H3, E4): the user, their identities, and the logins they sign in with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Caller {
    /// The user.
    pub user: UserId,
    /// Their identities, which "mine" covers together: those holding one of their verified
    /// emails. Usually one; two or more when they are duplicates of one person. At least one
    /// once the user has arrived.
    pub identities: BTreeSet<IdentityKey>,
    /// The logins they sign in with (ARCHITECTURE, Auth: users and logins), each with the
    /// name and verified emails it carried at its last sign-in, in provider and subject
    /// order.
    pub logins: Vec<LoginRecord>,
}

/// A user and the name people see for them (H3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserName {
    /// The user.
    pub user: UserId,
    /// Their name: their identity's name, else its first email; with no identity, the name
    /// their login gave, else its first verified email.
    pub name: Title,
}

impl Service {
    /// H3: the caller's identities, from the verified emails of every login they signed in
    /// with, matched against identity emails in the current deployment. Emails are stored
    /// trimmed and lower-cased, so the match is case-insensitive; an email the provider did
    /// not verify is never listed on a login, so it never matches. Reads only: an identity
    /// is created on arrival, never here.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn caller(&self, call: &Call) -> Result<Caller, ServiceError> {
        let user = call.actor.user.clone();
        let logins = sorted(self.store.logins_of(&user).await?);
        let deployment = self.deployment().await?;
        Ok(Caller {
            identities: identities_of(&logins, &deployment),
            user,
            logins,
        })
    }

    /// H3: a user's arrival, once per request after authentication and before any handler.
    /// The user has an identity when one holds any of their verified emails; when none does,
    /// one is created holding all of them, named by the name their login gave, if it gave
    /// one, through an ordinary deployment patch as the user (never as an agent acting for
    /// them). Handlers read the caller's identities and never create one.
    ///
    /// The key and the patch id are minted by `keys` afresh for each attempt, so two
    /// requests creating at once never share a patch id. Each email is held by at most one
    /// identity, so the loser of a race is rejected (stale, or the email is taken), looks
    /// again, and finds the winner's identity. An accepted create is looked for again too:
    /// a receipt proves a create committed, not that the identity is still there.
    ///
    /// Cost: one login read and one indexed email lookup; a deployment load only to create.
    ///
    /// # Errors
    ///
    /// The store fails, or the create is rejected on every attempt.
    pub async fn arrive(
        &self,
        call: &Call,
        keys: &mut (dyn KeyAllocator + Send),
    ) -> Result<(), WriteError> {
        let user = &call.actor.user;
        let mut rejected = None;
        for _ in 0..ARRIVAL_ATTEMPT_COUNT_MAX {
            let (logins, held) = self.matched(user).await?;
            if !held.is_empty() {
                return Ok(());
            }
            let identity = Identity {
                key: mint::<IdentityKey>(keys),
                name: logins.iter().find_map(|login| login.name.clone()),
                emails: logins
                    .iter()
                    .flat_map(|login| login.verified_emails.iter().cloned())
                    .collect(),
            };
            let created = creation(
                mint::<PatchId>(keys),
                identity,
                self.deployment().await?.revision,
            );
            let as_user = Call {
                actor: Actor {
                    user: user.clone(),
                    agent: None,
                },
                now: call.now,
            };
            match self.patch(&as_user, &created).await {
                Ok(_) => rejected = None,
                Err(WriteError::Rejected(rejection)) => {
                    rejected = Some(WriteError::Rejected(rejection));
                }
                Err(failed) => return Err(failed),
            }
        }
        match (self.matched(user).await?.1.is_empty(), rejected) {
            (true, Some(rejection)) => Err(rejection),
            _ => Ok(()),
        }
    }

    /// The user's logins and the identities holding any of their verified emails, found
    /// without loading the deployment.
    async fn matched(
        &self,
        user: &UserId,
    ) -> Result<(Vec<LoginRecord>, BTreeSet<IdentityKey>), ServiceError> {
        let logins = sorted(self.store.logins_of(user).await?);
        let emails: BTreeSet<Email> = logins
            .iter()
            .flat_map(|login| login.verified_emails.iter().cloned())
            .collect();
        let held = self.store.identities_with_emails(&emails).await?;
        Ok((logins, held))
    }

    /// H3: the names of `users`, in the order asked, leaving out users that do not exist or
    /// that have nothing to be called by. A user goes by their identity's name, else its
    /// first email (the first identity by key when their emails name several, which are
    /// duplicates of one person), else, with none left, by the name their login gave, else
    /// its first verified email. Nothing here ever answers an id as a name.
    ///
    /// # Errors
    ///
    /// [`ReadError::TooManyUsers`] for more users than a page holds; when the store fails.
    pub async fn user_names(&self, users: &[UserId]) -> Result<Vec<UserName>, ReadError> {
        // PRACTICES: Explicit limits.
        if !u32::try_from(users.len()).is_ok_and(|count| count <= PAGE_ITEM_COUNT_MAX) {
            return Err(ReadError::TooManyUsers { count: users.len() });
        }
        let deployment = self.deployment().await?;
        let mut names = Vec::with_capacity(users.len());
        for user in users {
            if self.store.user(user).await?.is_none() {
                continue;
            }
            let logins = sorted(self.store.logins_of(user).await?);
            let identity = identities_of(&logins, &deployment)
                .into_iter()
                .find_map(|key| deployment.identities.get(&key).map(Identity::display))
                .map(str::to_owned);
            let login = || {
                let from_login = logins.iter().find_map(|login| login.name.as_ref());
                let emailed = logins
                    .iter()
                    .find_map(|login| login.verified_emails.first())
                    .map(Email::as_str);
                from_login.map(Title::as_str).or(emailed).map(str::to_owned)
            };
            let name = identity
                .or_else(login)
                .and_then(|name| name.parse::<Title>().ok());
            if let Some(name) = name {
                names.push(UserName {
                    user: user.clone(),
                    name,
                });
            }
        }
        Ok(names)
    }
}

/// H3: the deployment patch that creates `identity`, committed as its user.
fn creation(id: PatchId, identity: Identity, base_revision: Revision) -> DomainPatch {
    let mutations = match Mutations::new(vec![Mutation::CreateIdentity { identity }]) {
        Ok(mutations) => mutations,
        Err(error) => unreachable!("one mutation is within the limit: {error}"),
    };
    let patch = Patch {
        id,
        target: PatchTarget::Deployment,
        base_revision,
        deployment_revision: None,
        mutations,
    };
    match DomainPatch::new(patch, None) {
        Ok(created) => created,
        Err(error) => unreachable!("a deployment patch is a domain's: {error:?}"),
    }
}

fn sorted(mut logins: Vec<LoginRecord>) -> Vec<LoginRecord> {
    logins.sort_by(|a, b| (&a.provider, &a.subject).cmp(&(&b.provider, &b.subject)));
    logins
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
