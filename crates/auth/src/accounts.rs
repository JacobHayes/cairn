//! Users, their logins, and browser sessions, over the store's auth records
//! (ARCHITECTURE, Auth; Store trait). Creating a user on first sign-in and linking an
//! login are auth operations, written to the auth log rather than to any domain (J2).

use std::collections::BTreeSet;
use std::sync::Arc;

use cairn_schema::{Login, Title, UserId};
use cairn_store::{
    AuthEvent, AuthLogEntry, AuthLogQuery, LoggedAuthEntry, LoginRecord, OAuthStateKind,
    OAuthStateRecord, Page, SessionRecord, Store, UserRecord,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
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

    /// H1, H3: the user a login signs in as. A known login is its user, with the name and
    /// verified emails the provider lists now; a new one joins the one user whose login has
    /// a verified email of its own, else is a new user (emails held by two users link to
    /// neither). A login with no verified email is refused: an identity is found by email,
    /// so a user without one could never be matched again (H3).
    ///
    /// # Errors
    ///
    /// The login has no verified email, or the store failed.
    pub async fn resolve(&self, login: &Login) -> Result<UserId, AuthError> {
        if login.verified_emails.is_empty() {
            return Err(AuthError::NoVerifiedEmail);
        }
        let (provider, subject) = (&login.provider, &login.subject);
        // Every request through a per-request provider resolves its login, so the usual
        // case, a known login unchanged since its last sign-in, takes no lock.
        let known = self.store.login(provider, subject).await?;
        if let Some(held) = known.filter(|held| unchanged(held, login)) {
            return Ok(held.user);
        }
        let _resolving = self.resolving.lock().await;
        if let Some(held) = self.store.login(provider, subject).await? {
            let user = held.user.clone();
            if !unchanged(&held, login) {
                let (name, verified_emails) = (login.name.clone(), login.verified_emails.clone());
                let refreshed = LoginRecord {
                    name,
                    verified_emails,
                    ..held
                };
                self.store.put_login(refreshed).await?;
            }
            return Ok(user);
        }
        let user = match self.holder(login).await? {
            Some(user) => user,
            None => self.create_user().await?,
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

    /// The one user whose logins hold a verified email of `login`'s, if exactly one does.
    async fn holder(&self, login: &Login) -> Result<Option<UserId>, AuthError> {
        let mut holders = BTreeSet::new();
        for email in &login.verified_emails {
            holders.extend(self.store.users_with_email(email).await?);
        }
        let mut holders = holders.into_iter();
        Ok(holders.next().filter(|_| holders.next().is_none()))
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

    /// Holds `pending` of `kind` under the digest of `secret` for `lifetime`: a step of a
    /// sign-in or an authorization that its secret's holder finishes once.
    ///
    /// # Errors
    ///
    /// `pending` is too large to hold, or the store failed.
    pub async fn hold<T: Serialize>(
        &self,
        secret: &str,
        kind: OAuthStateKind,
        pending: &T,
        lifetime: std::time::Duration,
    ) -> Result<(), AuthError> {
        let payload = serde_json::to_string(pending)
            .ok()
            .and_then(|json| json.parse().ok())
            .ok_or_else(|| AuthError::Upstream("the request is too large to hold".to_owned()))?;
        let record = OAuthStateRecord {
            key_hash: digest(secret),
            kind,
            payload,
            expires_at: self.clock.after(lifetime),
        };
        Ok(self.store.put_oauth_state(record).await?)
    }

    /// Takes what [`Accounts::hold`] holds for `secret`: used once, whatever comes of it.
    ///
    /// # Errors
    ///
    /// The store failed.
    pub async fn take<T: DeserializeOwned>(
        &self,
        secret: &str,
        kind: OAuthStateKind,
    ) -> Result<Option<T>, AuthError> {
        let now = self.clock.now();
        let record = self.store.take_oauth_state(&digest(secret), now).await?;
        let record = record.filter(|record| record.kind == kind);
        Ok(record.and_then(|record| serde_json::from_str(record.payload.as_str()).ok()))
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

/// How the auth log names a login: `provider:subject`, or the subject alone when the
/// pair would pass the title limit (a subject is already a title).
fn login_subject(login: &Login) -> Title {
    format!("{}:{}", login.provider, login.subject)
        .parse()
        .unwrap_or_else(|_| login.subject.clone())
}
