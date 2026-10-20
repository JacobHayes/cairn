//! Users, their logins, and browser sessions, over the store's auth records
//! (ARCHITECTURE, Auth; Store trait). Creating a user on first sign-in and linking an
//! login are auth operations, written to the auth log rather than to any domain (J2).

use std::collections::BTreeSet;
use std::sync::Arc;

use cairn_schema::{Login, Title, UserId};
use cairn_store::{
    AuthEvent, AuthLogEntry, AuthLogQuery, LoggedAuthEntry, LoginRecord, Page, SessionRecord,
    Store, UserRecord,
};
use tokio::sync::Mutex;

use crate::clock::Clock;
use crate::error::AuthError;
use crate::lifetimes::SESSION_LIFETIME;
use crate::secret::{Secret, SecretKind, digest, mint_id};

/// Users, logins, and sessions over one store.
#[derive(Clone)]
pub struct Accounts {
    store: Arc<dyn Store>,
    clock: Clock,
    /// Resolution is a read then writes; one at a time, so two first sign-ins of one
    /// login cannot both create a user. One process serves a deployment (ARCHITECTURE,
    /// Concurrency and notification), so a lock in the process is enough.
    resolving: Arc<Mutex<()>>,
}

impl Accounts {
    /// Accounts over `store`, reading `clock`.
    #[must_use]
    pub fn new(store: Arc<dyn Store>, clock: Clock) -> Self {
        Self {
            store,
            clock,
            resolving: Arc::new(Mutex::new(())),
        }
    }

    /// The store.
    #[must_use]
    pub fn store(&self) -> &dyn Store {
        self.store.as_ref()
    }

    /// The clock.
    #[must_use]
    pub fn clock(&self) -> &Clock {
        &self.clock
    }

    /// H1, H3: the user a login signs in as. A known login is its user, with the
    /// verified emails the provider lists now. A new one is linked to `signed_in` when the
    /// browser is already signed in (linking another provider), else, with `auto_link`, to
    /// the one user holding a login with one of its verified emails, else to a new
    /// user. A login held by another user than `signed_in` is refused: merging users
    /// is `Later` (PRD). A login with no verified email is refused: an identity is found by
    /// email, so a user without one could never be matched again (H3).
    ///
    /// # Errors
    ///
    /// The login has no verified email, it is held by another user, or the store failed.
    pub async fn resolve(
        &self,
        login: &Login,
        signed_in: Option<&UserId>,
        auto_link: bool,
    ) -> Result<UserId, AuthError> {
        if login.verified_emails.is_empty() {
            return Err(AuthError::NoVerifiedEmail);
        }
        let (provider, subject) = (&login.provider, &login.subject);
        // Every request through a per-request provider resolves its login, so the usual
        // case, a known login unchanged since its last sign-in, takes no lock.
        let known = self.store.login(provider, subject).await?;
        if let Some(held) = known.filter(|held| unchanged(held, login)) {
            return holder(held, signed_in);
        }
        let _resolving = self.resolving.lock().await;
        if let Some(held) = self.store.login(provider, subject).await? {
            return self.refresh(held, login, signed_in).await;
        }
        let user = match signed_in {
            Some(user) => user.clone(),
            None => match self.auto_link_target(login, auto_link).await? {
                Some(user) => user,
                None => self.create_user().await?,
            },
        };
        self.store
            .put_login(LoginRecord {
                provider: login.provider.clone(),
                subject: login.subject.clone(),
                user: user.clone(),
                name: login.name.clone(),
                verified_emails: login.verified_emails.clone(),
                linked_at: self.clock.now(),
            })
            .await?;
        self.log(AuthEvent::LoginLinked, &user, Some(login_subject(login)))
            .await?;
        Ok(user)
    }

    /// A known login's user, after recording the name and emails its provider gives now.
    async fn refresh(
        &self,
        held: LoginRecord,
        login: &Login,
        signed_in: Option<&UserId>,
    ) -> Result<UserId, AuthError> {
        let user = holder(held.clone(), signed_in)?;
        if !unchanged(&held, login) {
            self.store
                .put_login(LoginRecord {
                    name: login.name.clone(),
                    verified_emails: login.verified_emails.clone(),
                    ..held
                })
                .await?;
        }
        Ok(user)
    }

    /// H3: the one user holding a login with one of `login`'s verified emails, when
    /// the provider auto-links. None when emails point at two users: that is a person with
    /// two accounts, which is not for a sign-in to guess between.
    async fn auto_link_target(
        &self,
        login: &Login,
        auto_link: bool,
    ) -> Result<Option<UserId>, AuthError> {
        if !auto_link {
            return Ok(None);
        }
        let mut users = BTreeSet::new();
        for email in &login.verified_emails {
            for held in self.store.logins_with_email(email).await? {
                users.insert(held.user);
            }
        }
        let mut users = users.into_iter();
        Ok(match (users.next(), users.next()) {
            (Some(user), None) => Some(user),
            _ => None,
        })
    }

    async fn create_user(&self) -> Result<UserId, AuthError> {
        let user: UserId = mint_id()?;
        self.store
            .put_user(UserRecord {
                id: user.clone(),
                created_at: self.clock.now(),
            })
            .await?;
        self.log(AuthEvent::UserCreated, &user, None).await?;
        Ok(user)
    }

    /// Whether `user` has a login with a verified email, as every sign-in now requires
    /// (H3): a session or agent token of a user without one, which predates the rule, is
    /// not honoured, so they sign in again.
    ///
    /// # Errors
    ///
    /// The store failed.
    pub async fn can_be_matched(&self, user: &UserId) -> Result<bool, AuthError> {
        let logins = self.store.logins_of(user).await?;
        Ok(logins.iter().any(|login| !login.verified_emails.is_empty()))
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

/// Whether a stored login already says what its provider says now.
fn unchanged(held: &LoginRecord, login: &Login) -> bool {
    held.name == login.name && held.verified_emails == login.verified_emails
}

/// The user holding a known login, unless the browser is signed in as someone else.
fn holder(held: LoginRecord, signed_in: Option<&UserId>) -> Result<UserId, AuthError> {
    match signed_in {
        Some(user) if *user != held.user => {
            Err(AuthError::LoginHeldByAnotherUser { holder: held.user })
        }
        _ => Ok(held.user),
    }
}

/// How the auth log names a login: `provider:subject`, or the subject alone when the
/// pair would pass the title limit (a subject is already a title).
fn login_subject(login: &Login) -> Title {
    format!("{}:{}", login.provider, login.subject)
        .parse()
        .unwrap_or_else(|_| login.subject.clone())
}
