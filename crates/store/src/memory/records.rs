//! The memory backend's records outside every domain.

use std::collections::BTreeMap;

use cairn_schema::{AgentId, ConversationId, Slug, Timestamp, Title, UserId};

use crate::backend;
use crate::commit::StoreError;
use crate::query::{Page, PageSize};
use crate::records::{
    AgentTokenRecord, AuthLogEntry, AuthLogQuery, ConversationRecord, ConversationSummary,
    IdentityRecord, LoggedAuthEntry, OAuthStateRecord, SecretHash, SessionRecord, UserRecord,
};

/// Auth state and conversations.
#[derive(Debug, Default)]
pub(super) struct Outside {
    users: BTreeMap<UserId, UserRecord>,
    identities: BTreeMap<(Slug, Title), IdentityRecord>,
    sessions: BTreeMap<SecretHash, SessionRecord>,
    oauth: BTreeMap<SecretHash, OAuthStateRecord>,
    tokens: BTreeMap<AgentId, AgentTokenRecord>,
    auth_log: Vec<LoggedAuthEntry>,
    conversations: BTreeMap<ConversationId, ConversationRecord>,
}

impl Outside {
    fn user_exists(&self, user: &UserId) -> Result<(), StoreError> {
        if self.users.contains_key(user) {
            Ok(())
        } else {
            Err(backend::no_such_user(user))
        }
    }

    pub fn put_user(&mut self, user: UserRecord) {
        self.users.insert(user.id.clone(), user);
    }

    pub fn user(&self, id: &UserId) -> Option<UserRecord> {
        self.users.get(id).cloned()
    }

    pub fn put_identity(&mut self, identity: IdentityRecord) -> Result<(), StoreError> {
        self.user_exists(&identity.user)?;
        let key = (identity.provider.clone(), identity.subject.clone());
        self.identities.insert(key, identity);
        Ok(())
    }

    pub fn identity(&self, provider: &Slug, subject: &Title) -> Option<IdentityRecord> {
        self.identities
            .get(&(provider.clone(), subject.clone()))
            .cloned()
    }

    pub fn identities_of(&self, user: &UserId) -> Vec<IdentityRecord> {
        let mine = self
            .identities
            .values()
            .filter(|identity| identity.user == *user);
        mine.cloned().collect()
    }

    pub fn remove_identity(&mut self, provider: &Slug, subject: &Title) -> bool {
        self.identities
            .remove(&(provider.clone(), subject.clone()))
            .is_some()
    }

    pub fn put_session(&mut self, session: SessionRecord) -> Result<(), StoreError> {
        self.user_exists(&session.user)?;
        self.sessions.insert(session.token_hash.clone(), session);
        Ok(())
    }

    pub fn session(&self, token_hash: &SecretHash, now: Timestamp) -> Option<SessionRecord> {
        let session = self.sessions.get(token_hash)?;
        (session.expires_at > now).then(|| session.clone())
    }

    pub fn remove_session(&mut self, token_hash: &SecretHash) -> bool {
        self.sessions.remove(token_hash).is_some()
    }

    pub fn put_oauth_state(&mut self, state: OAuthStateRecord) {
        self.oauth.insert(state.key_hash.clone(), state);
    }

    pub fn take_oauth_state(
        &mut self,
        key_hash: &SecretHash,
        now: Timestamp,
    ) -> Option<OAuthStateRecord> {
        let state = self.oauth.remove(key_hash)?;
        (state.expires_at > now).then_some(state)
    }

    pub fn remove_expired(&mut self, now: Timestamp) -> u64 {
        let before = self.sessions.len() + self.oauth.len();
        self.sessions.retain(|_, session| session.expires_at > now);
        self.oauth.retain(|_, state| state.expires_at > now);
        let removed = before - (self.sessions.len() + self.oauth.len());
        u64::try_from(removed).unwrap_or(u64::MAX)
    }

    pub fn put_agent_token(&mut self, token: AgentTokenRecord) -> Result<(), StoreError> {
        self.user_exists(&token.user)?;
        let digest_held = self
            .tokens
            .values()
            .any(|held| held.token_hash == token.token_hash && held.agent != token.agent);
        if digest_held {
            return Err(StoreError::Malformed(format!(
                "another agent token has the digest of {}'s",
                token.agent
            )));
        }
        self.tokens.insert(token.agent.clone(), token);
        Ok(())
    }

    pub fn agent_token(&self, token_hash: &SecretHash) -> Option<AgentTokenRecord> {
        let token = self
            .tokens
            .values()
            .find(|held| held.token_hash == *token_hash);
        token.cloned()
    }

    pub fn agent_tokens_of(&self, user: &UserId) -> Vec<AgentTokenRecord> {
        let mine = self.tokens.values().filter(|token| token.user == *user);
        mine.cloned().collect()
    }

    pub fn append_auth_log(&mut self, entry: AuthLogEntry) -> u64 {
        let seq = self.auth_log.last().map_or(0, |logged| logged.seq + 1);
        self.auth_log.push(LoggedAuthEntry { seq, entry });
        seq
    }

    pub fn auth_log(&self, query: &AuthLogQuery) -> Page<LoggedAuthEntry, u64> {
        let items: Vec<_> = self
            .auth_log
            .iter()
            .filter(|logged| query.after.is_none_or(|after| logged.seq > after))
            .filter(|logged| {
                query
                    .user
                    .as_ref()
                    .is_none_or(|user| logged.entry.user == *user)
            })
            .take(query.size.len() + 1)
            .cloned()
            .collect();
        Page::cut(items, query.size, |logged| logged.seq)
    }

    pub fn put_conversation(&mut self, conversation: ConversationRecord) -> Result<(), StoreError> {
        backend::check_conversation_size(&conversation)?;
        self.conversations
            .insert(conversation.id.clone(), conversation);
        Ok(())
    }

    pub fn conversation(&self, id: &ConversationId) -> Option<ConversationRecord> {
        self.conversations.get(id).cloned()
    }

    pub fn conversations_of(
        &self,
        user: &UserId,
        after: Option<&ConversationId>,
        size: PageSize,
    ) -> Page<ConversationSummary, ConversationId> {
        let items: Vec<_> = self
            .conversations
            .values()
            .filter(|conversation| conversation.user == *user)
            .filter(|conversation| after.is_none_or(|after| conversation.id > *after))
            .take(size.len() + 1)
            .map(ConversationRecord::summary)
            .collect();
        Page::cut(items, size, |summary| summary.id.clone())
    }

    pub fn remove_conversation(&mut self, id: &ConversationId) -> bool {
        self.conversations.remove(id).is_some()
    }
}
