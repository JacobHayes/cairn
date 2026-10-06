//! Agent tokens (ARCHITECTURE, Auth; Store trait): long-lived bearer tokens a user mints
//! for a script or an MCP client. A token is shown once when minted and only its digest is
//! stored; it names an agent acting for its user (H2) until it is revoked. Minting and
//! revoking are auth operations, written to the auth log.

use cairn_schema::{Actor, AgentId, Timestamp, Title, UserId};
use cairn_store::{AgentTokenRecord, AuthEvent, AuthStore};

use crate::accounts::Accounts;
use crate::error::AuthError;
use crate::secret::{Secret, SecretKind, digest, mint_id};

/// A token just minted: the one time its secret is seen.
#[derive(Debug)]
pub struct MintedToken {
    /// The agent the token acts as.
    pub agent: AgentId,
    /// The token, to hand to its holder now; Cairn keeps only its digest.
    pub token: Secret,
}

/// An agent token as its user's list shows it: never the secret, nor its digest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentToken {
    /// The agent the token acts as.
    pub agent: AgentId,
    /// What the user called it.
    pub name: Title,
    /// When it was minted.
    pub created_at: Timestamp,
    /// When it was revoked, if it was.
    pub revoked_at: Option<Timestamp>,
}

impl<S: AuthStore> Accounts<S> {
    /// H2: mints a token for a new agent acting for `actor`'s user, named `name`. Only a
    /// user mints tokens: an agent cannot create other agents.
    ///
    /// # Errors
    ///
    /// The actor is an agent, or the random source or the store failed.
    pub async fn mint_token(&self, actor: &Actor, name: Title) -> Result<MintedToken, AuthError> {
        if actor.agent.is_some() {
            return Err(AuthError::AgentNotAllowed);
        }
        let agent: AgentId = mint_id()?;
        let token = Secret::mint(SecretKind::Agent)?;
        self.store()
            .put_agent_token(AgentTokenRecord {
                agent: agent.clone(),
                token_hash: token.digest(),
                name,
                user: actor.user.clone(),
                created_at: self.clock().now(),
                revoked_at: None,
            })
            .await?;
        self.log(
            AuthEvent::TokenMinted,
            &actor.user,
            Some(agent_subject(&agent)),
        )
        .await?;
        Ok(MintedToken { agent, token })
    }

    /// A user's agent tokens, revoked ones included, by agent id.
    ///
    /// # Errors
    ///
    /// The store failed.
    pub async fn tokens_of(&self, user: &UserId) -> Result<Vec<AgentToken>, AuthError> {
        let records = self.store().agent_tokens_of(user).await?;
        let tokens = records.into_iter().map(|record| AgentToken {
            agent: record.agent,
            name: record.name,
            created_at: record.created_at,
            revoked_at: record.revoked_at,
        });
        Ok(tokens.collect())
    }

    /// Revokes one of `actor`'s user's tokens; from now on it is refused. Revoking a
    /// revoked token changes nothing. Only a user revokes tokens.
    ///
    /// # Errors
    ///
    /// The actor is an agent, the token is not one of the user's, or the store failed.
    pub async fn revoke_token(&self, actor: &Actor, agent: &AgentId) -> Result<(), AuthError> {
        if actor.agent.is_some() {
            return Err(AuthError::AgentNotAllowed);
        }
        let records = self.store().agent_tokens_of(&actor.user).await?;
        let Some(record) = records.into_iter().find(|record| record.agent == *agent) else {
            return Err(AuthError::NoSuchToken);
        };
        if record.revoked_at.is_some() {
            return Ok(());
        }
        let revoked_at = Some(self.clock().now());
        self.store()
            .put_agent_token(AgentTokenRecord {
                revoked_at,
                ..record
            })
            .await?;
        self.log(
            AuthEvent::TokenRevoked,
            &actor.user,
            Some(agent_subject(agent)),
        )
        .await
    }

    /// H2: the actor a presented agent token names, the agent set; none for a token that is
    /// not a live agent token.
    ///
    /// # Errors
    ///
    /// The store failed.
    pub async fn token_actor(&self, token: &str) -> Result<Option<Actor>, AuthError> {
        if !SecretKind::Agent.matches(token) {
            return Ok(None);
        }
        let Some(record) = self.store().agent_token(&digest(token)).await? else {
            return Ok(None);
        };
        Ok(record.revoked_at.is_none().then_some(Actor {
            user: record.user,
            agent: Some(record.agent),
        }))
    }
}

/// How the auth log names a token: its agent id.
fn agent_subject(agent: &AgentId) -> Title {
    match agent.as_str().parse() {
        Ok(title) => title,
        Err(error) => unreachable!("an agent id is a title: {error}"),
    }
}
