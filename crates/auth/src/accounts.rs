//! Users, their identities, and browser sessions, over the store's auth records
//! (ARCHITECTURE, Auth; Store trait). Creating a user on first sign-in and linking an
//! identity are auth operations, written to the auth log rather than to any domain (J2).

use std::collections::BTreeSet;
use std::sync::Arc;

use cairn_schema::{Identity, Title, UserId};
use cairn_store::{
    AuthEvent, AuthLogEntry, AuthLogQuery, AuthStore, IdentityRecord, LoggedAuthEntry, Page,
    SessionRecord, UserRecord,
};
use tokio::sync::Mutex;

use crate::clock::Clock;
use crate::error::AuthError;
use crate::lifetimes::SESSION_LIFETIME;
use crate::secret::{Secret, SecretKind, digest, mint_id};

/// Users, identities, and sessions over one store.
pub struct Accounts<S> {
    store: Arc<S>,
    clock: Clock,
    /// Resolution is a read then writes; one at a time, so two first sign-ins of one
    /// identity cannot both create a user. One process serves a deployment (ARCHITECTURE,
    /// Concurrency and notification), so a lock in the process is enough.
    resolving: Arc<Mutex<()>>,
}

impl<S> Clone for Accounts<S> {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
            clock: self.clock.clone(),
            resolving: Arc::clone(&self.resolving),
        }
    }
}

impl<S: AuthStore> Accounts<S> {
    /// Accounts over `store`, reading `clock`.
    #[must_use]
    pub fn new(store: Arc<S>, clock: Clock) -> Self {
        Self {
            store,
            clock,
            resolving: Arc::new(Mutex::new(())),
        }
    }

    /// The store.
    #[must_use]
    pub fn store(&self) -> &S {
        &self.store
    }

    /// The clock.
    #[must_use]
    pub fn clock(&self) -> &Clock {
        &self.clock
    }

    /// H1, H3: the user an identity signs in as. A known identity is its user, with the
    /// verified emails the provider lists now. A new one is linked to `signed_in` when the
    /// browser is already signed in (linking another provider), else, with `auto_link`, to
    /// the one user holding an identity with one of its verified emails, else to a new
    /// user. An identity held by another user than `signed_in` is refused: merging users
    /// is `Later` (PRD).
    ///
    /// # Errors
    ///
    /// The identity is held by another user, or the store failed.
    pub async fn resolve(
        &self,
        identity: &Identity,
        signed_in: Option<&UserId>,
        auto_link: bool,
    ) -> Result<UserId, AuthError> {
        let (provider, subject) = (&identity.provider, &identity.subject);
        // Every request through a per-request provider resolves its identity, so the usual
        // case, a known identity whose emails are unchanged, takes no lock.
        let known = self.store.identity(provider, subject).await?;
        if let Some(held) = known.filter(|held| held.verified_emails == identity.verified_emails) {
            return holder(held, signed_in);
        }
        let _resolving = self.resolving.lock().await;
        if let Some(held) = self.store.identity(provider, subject).await? {
            return self.refresh(held, identity, signed_in).await;
        }
        let user = match signed_in {
            Some(user) => user.clone(),
            None => match self.auto_link_target(identity, auto_link).await? {
                Some(user) => user,
                None => self.create_user(identity).await?,
            },
        };
        self.store
            .put_identity(IdentityRecord {
                provider: identity.provider.clone(),
                subject: identity.subject.clone(),
                user: user.clone(),
                verified_emails: identity.verified_emails.clone(),
                linked_at: self.clock.now(),
            })
            .await?;
        self.log(
            AuthEvent::IdentityLinked,
            &user,
            Some(identity_subject(identity)),
        )
        .await?;
        Ok(user)
    }

    /// A known identity's user, after recording the emails its provider verifies now.
    async fn refresh(
        &self,
        held: IdentityRecord,
        identity: &Identity,
        signed_in: Option<&UserId>,
    ) -> Result<UserId, AuthError> {
        let user = holder(held.clone(), signed_in)?;
        if held.verified_emails != identity.verified_emails {
            let verified_emails = identity.verified_emails.clone();
            self.store
                .put_identity(IdentityRecord {
                    verified_emails,
                    ..held
                })
                .await?;
        }
        Ok(user)
    }

    /// H3: the one user holding an identity with one of `identity`'s verified emails, when
    /// the provider auto-links. None when emails point at two users: that is a person with
    /// two accounts, which is not for a sign-in to guess between.
    async fn auto_link_target(
        &self,
        identity: &Identity,
        auto_link: bool,
    ) -> Result<Option<UserId>, AuthError> {
        if !auto_link {
            return Ok(None);
        }
        let mut users = BTreeSet::new();
        for email in &identity.verified_emails {
            for held in self.store.identities_with_email(email).await? {
                users.insert(held.user);
            }
        }
        let mut users = users.into_iter();
        Ok(match (users.next(), users.next()) {
            (Some(user), None) => Some(user),
            _ => None,
        })
    }

    async fn create_user(&self, identity: &Identity) -> Result<UserId, AuthError> {
        let user: UserId = mint_id()?;
        self.store
            .put_user(UserRecord {
                id: user.clone(),
                name: identity.display.clone(),
                created_at: self.clock.now(),
            })
            .await?;
        self.log(AuthEvent::UserCreated, &user, None).await?;
        Ok(user)
    }

    /// Starts a browser session for `user`; the token is the cookie's value, handed over
    /// once and stored only as its digest.
    ///
    /// # Errors
    ///
    /// The random source or the store failed.
    pub async fn start_session(&self, user: &UserId) -> Result<Secret, AuthError> {
        let token = Secret::mint(SecretKind::Session)?;
        self.store
            .put_session(SessionRecord {
                token_hash: token.digest(),
                user: user.clone(),
                created_at: self.clock.now(),
                expires_at: self.clock.after(SESSION_LIFETIME),
            })
            .await?;
        Ok(token)
    }

    /// The user a session token signs in as; none for a token that is not a live session.
    ///
    /// # Errors
    ///
    /// The store failed.
    pub async fn session_user(&self, token: &str) -> Result<Option<UserId>, AuthError> {
        if !SecretKind::Session.matches(token) {
            return Ok(None);
        }
        let session = self.store.session(&digest(token), self.clock.now()).await?;
        Ok(session.map(|session| session.user))
    }

    /// Ends a session.
    ///
    /// # Errors
    ///
    /// The store failed.
    pub async fn end_session(&self, token: &str) -> Result<(), AuthError> {
        if SecretKind::Session.matches(token) {
            self.store.remove_session(&digest(token)).await?;
        }
        Ok(())
    }

    /// The auth log, filtered and paged.
    ///
    /// # Errors
    ///
    /// The store failed.
    pub async fn auth_log(
        &self,
        query: &AuthLogQuery,
    ) -> Result<Page<LoggedAuthEntry, u64>, AuthError> {
        Ok(self.store.auth_log(query).await?)
    }

    pub(crate) async fn log(
        &self,
        event: AuthEvent,
        user: &UserId,
        subject: Option<Title>,
    ) -> Result<(), AuthError> {
        let entry = AuthLogEntry {
            at: self.clock.now(),
            event,
            user: user.clone(),
            subject,
        };
        self.store.append_auth_log(entry).await?;
        Ok(())
    }
}

/// The user holding a known identity, unless the browser is signed in as someone else.
fn holder(held: IdentityRecord, signed_in: Option<&UserId>) -> Result<UserId, AuthError> {
    match signed_in {
        Some(user) if *user != held.user => {
            Err(AuthError::IdentityHeldByAnotherUser { holder: held.user })
        }
        _ => Ok(held.user),
    }
}

/// How the auth log names an identity: `provider:subject`, or the subject alone when the
/// pair would pass the title limit (a subject is already a title).
fn identity_subject(identity: &Identity) -> Title {
    format!("{}:{}", identity.provider, identity.subject)
        .parse()
        .unwrap_or_else(|_| identity.subject.clone())
}
