//! Auth state and conversations: plain rows outside every domain, answering as the memory
//! backend does.

use std::collections::BTreeSet;

use cairn_schema::{ConversationId, Slug, Timestamp, Title, UserId};
use cairn_store::backend;
use cairn_store::{
    AgentTokenRecord, AuthLogEntry, AuthLogQuery, ConversationRecord, ConversationSummary,
    IdentityRecord, LoggedAuthEntry, OAuthStateRecord, Page, PageSize, SecretHash, SessionRecord,
    StoreError, UserRecord,
};
use serde_json::json;
use turso::{Connection, Value};

use crate::sql::{Row, corrupt, execute, first, int, opt_text, rows, text, time};
use crate::write::json_enum;

fn enum_from<T: serde::de::DeserializeOwned>(name: &str) -> Result<T, StoreError> {
    serde_json::from_value(json!(name)).map_err(|error| corrupt(&format!("{error}")))
}

async fn user_exists(connection: &Connection, user: &UserId) -> Result<(), StoreError> {
    match first(
        connection,
        "SELECT 1 FROM users WHERE id = ?1",
        vec![text(user)],
    )
    .await?
    {
        Some(_) => Ok(()),
        None => Err(backend::no_such_user(user)),
    }
}

pub(crate) async fn put_user(connection: &Connection, user: &UserRecord) -> Result<(), StoreError> {
    execute(
        connection,
        "INSERT OR REPLACE INTO users (id, name, created_at) VALUES (?1, ?2, ?3)",
        vec![text(&user.id), text(&user.name), time(user.created_at)?],
    )
    .await?;
    Ok(())
}

pub(crate) async fn user(
    connection: &Connection,
    id: &UserId,
) -> Result<Option<UserRecord>, StoreError> {
    let select = "SELECT name, created_at FROM users WHERE id = ?1";
    let Some(row) = first(connection, select, vec![text(id)]).await? else {
        return Ok(None);
    };
    Ok(Some(UserRecord {
        id: id.clone(),
        name: row.parse(0)?,
        created_at: row.timestamp(1)?,
    }))
}

pub(crate) async fn put_identity(
    connection: &Connection,
    identity: &IdentityRecord,
) -> Result<(), StoreError> {
    user_exists(connection, &identity.user).await?;
    let key = || vec![text(&identity.provider), text(&identity.subject)];
    execute(
        connection,
        "INSERT OR REPLACE INTO user_identities (provider, subject, user_id, linked_at) \
         VALUES (?1, ?2, ?3, ?4)",
        vec![
            text(&identity.provider),
            text(&identity.subject),
            text(&identity.user),
            time(identity.linked_at)?,
        ],
    )
    .await?;
    execute(
        connection,
        "DELETE FROM identity_emails WHERE provider = ?1 AND subject = ?2",
        key(),
    )
    .await?;
    for email in &identity.verified_emails {
        let mut params = key();
        params.push(text(email));
        execute(
            connection,
            "INSERT INTO identity_emails (provider, subject, email) VALUES (?1, ?2, ?3)",
            params,
        )
        .await?;
    }
    Ok(())
}

async fn identities(
    connection: &Connection,
    filter: &str,
    params: Vec<Value>,
) -> Result<Vec<IdentityRecord>, StoreError> {
    let select = format!(
        "SELECT provider, subject, user_id, linked_at FROM user_identities WHERE {filter} \
         ORDER BY provider, subject"
    );
    let mut found = Vec::new();
    for row in rows(connection, &select, params).await? {
        let select = "SELECT email FROM identity_emails WHERE provider = ?1 AND subject = ?2";
        let mut verified_emails = BTreeSet::new();
        for email in rows(
            connection,
            select,
            vec![text(&row.text(0)?), text(&row.text(1)?)],
        )
        .await?
        {
            verified_emails.insert(email.parse(0)?);
        }
        found.push(IdentityRecord {
            provider: row.parse(0)?,
            subject: row.parse(1)?,
            user: row.parse(2)?,
            verified_emails,
            linked_at: row.timestamp(3)?,
        });
    }
    Ok(found)
}

pub(crate) async fn identity(
    connection: &Connection,
    provider: &Slug,
    subject: &Title,
) -> Result<Option<IdentityRecord>, StoreError> {
    let filter = "provider = ?1 AND subject = ?2";
    let found = identities(connection, filter, vec![text(provider), text(subject)]).await?;
    Ok(found.into_iter().next())
}

pub(crate) async fn identities_of(
    connection: &Connection,
    user: &UserId,
) -> Result<Vec<IdentityRecord>, StoreError> {
    identities(connection, "user_id = ?1", vec![text(user)]).await
}

pub(crate) async fn remove_identity(
    connection: &Connection,
    provider: &Slug,
    subject: &Title,
) -> Result<bool, StoreError> {
    let key = || vec![text(provider), text(subject)];
    execute(
        connection,
        "DELETE FROM identity_emails WHERE provider = ?1 AND subject = ?2",
        key(),
    )
    .await?;
    let removed = execute(
        connection,
        "DELETE FROM user_identities WHERE provider = ?1 AND subject = ?2",
        key(),
    )
    .await?;
    Ok(removed > 0)
}

pub(crate) async fn put_session(
    connection: &Connection,
    session: &SessionRecord,
) -> Result<(), StoreError> {
    user_exists(connection, &session.user).await?;
    execute(
        connection,
        "INSERT OR REPLACE INTO sessions (token_hash, user_id, created_at, expires_at) \
         VALUES (?1, ?2, ?3, ?4)",
        vec![
            text(&session.token_hash),
            text(&session.user),
            time(session.created_at)?,
            time(session.expires_at)?,
        ],
    )
    .await?;
    Ok(())
}

pub(crate) async fn session(
    connection: &Connection,
    token_hash: &SecretHash,
    now: Timestamp,
) -> Result<Option<SessionRecord>, StoreError> {
    let select = "SELECT user_id, created_at, expires_at FROM sessions \
                  WHERE token_hash = ?1 AND expires_at > ?2";
    let Some(row) = first(connection, select, vec![text(token_hash), time(now)?]).await? else {
        return Ok(None);
    };
    Ok(Some(SessionRecord {
        token_hash: token_hash.clone(),
        user: row.parse(0)?,
        created_at: row.timestamp(1)?,
        expires_at: row.timestamp(2)?,
    }))
}

pub(crate) async fn remove_session(
    connection: &Connection,
    token_hash: &SecretHash,
) -> Result<bool, StoreError> {
    let sql = "DELETE FROM sessions WHERE token_hash = ?1";
    Ok(execute(connection, sql, vec![text(token_hash)]).await? > 0)
}

pub(crate) async fn put_oauth_state(
    connection: &Connection,
    state: &OAuthStateRecord,
) -> Result<(), StoreError> {
    execute(
        connection,
        "INSERT OR REPLACE INTO oauth_transient (key_hash, kind, payload, expires_at) \
         VALUES (?1, ?2, ?3, ?4)",
        vec![
            text(&state.key_hash),
            json_enum(&state.kind)?,
            text(&state.payload),
            time(state.expires_at)?,
        ],
    )
    .await?;
    Ok(())
}

pub(crate) async fn take_oauth_state(
    connection: &Connection,
    key_hash: &SecretHash,
    now: Timestamp,
) -> Result<Option<OAuthStateRecord>, StoreError> {
    let select = "SELECT kind, payload, expires_at FROM oauth_transient WHERE key_hash = ?1";
    let Some(row) = first(connection, select, vec![text(key_hash)]).await? else {
        return Ok(None);
    };
    let sql = "DELETE FROM oauth_transient WHERE key_hash = ?1";
    execute(connection, sql, vec![text(key_hash)]).await?;
    let state = OAuthStateRecord {
        key_hash: key_hash.clone(),
        kind: enum_from(&row.text(0)?)?,
        payload: row.parse(1)?,
        expires_at: row.timestamp(2)?,
    };
    Ok((state.expires_at > now).then_some(state))
}

pub(crate) async fn remove_expired(
    connection: &Connection,
    now: Timestamp,
) -> Result<u64, StoreError> {
    let mut removed = 0;
    for table in ["sessions", "oauth_transient"] {
        let sql = format!("DELETE FROM {table} WHERE expires_at <= ?1");
        removed += execute(connection, &sql, vec![time(now)?]).await?;
    }
    Ok(removed)
}

pub(crate) async fn put_agent_token(
    connection: &Connection,
    token: &AgentTokenRecord,
) -> Result<(), StoreError> {
    user_exists(connection, &token.user).await?;
    let select = "SELECT 1 FROM agent_tokens WHERE token_hash = ?1 AND agent <> ?2";
    if first(
        connection,
        select,
        vec![text(&token.token_hash), text(&token.agent)],
    )
    .await?
    .is_some()
    {
        return Err(StoreError::Malformed(format!(
            "another agent token has the digest of {}'s",
            token.agent
        )));
    }
    execute(
        connection,
        "INSERT OR REPLACE INTO agent_tokens (agent, token_hash, name, user_id, created_at, \
         revoked_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        vec![
            text(&token.agent),
            text(&token.token_hash),
            text(&token.name),
            text(&token.user),
            time(token.created_at)?,
            token
                .revoked_at
                .map(time)
                .transpose()?
                .unwrap_or(Value::Null),
        ],
    )
    .await?;
    Ok(())
}

fn agent_token(row: &Row) -> Result<AgentTokenRecord, StoreError> {
    Ok(AgentTokenRecord {
        agent: row.parse(0)?,
        token_hash: row.parse(1)?,
        name: row.parse(2)?,
        user: row.parse(3)?,
        created_at: row.timestamp(4)?,
        revoked_at: row.opt_int(5)?.map(crate::sql::timestamp).transpose()?,
    })
}

const TOKEN_COLUMNS: &str = "agent, token_hash, name, user_id, created_at, revoked_at";

pub(crate) async fn agent_token_by_hash(
    connection: &Connection,
    token_hash: &SecretHash,
) -> Result<Option<AgentTokenRecord>, StoreError> {
    let select = format!("SELECT {TOKEN_COLUMNS} FROM agent_tokens WHERE token_hash = ?1");
    first(connection, &select, vec![text(token_hash)])
        .await?
        .map(|row| agent_token(&row))
        .transpose()
}

pub(crate) async fn agent_tokens_of(
    connection: &Connection,
    user: &UserId,
) -> Result<Vec<AgentTokenRecord>, StoreError> {
    let select =
        format!("SELECT {TOKEN_COLUMNS} FROM agent_tokens WHERE user_id = ?1 ORDER BY agent");
    rows(connection, &select, vec![text(user)])
        .await?
        .iter()
        .map(agent_token)
        .collect()
}

/// Appends at `seq`, a position the caller holds (`crate::sequence`).
pub(crate) async fn append_auth_log(
    connection: &Connection,
    entry: &AuthLogEntry,
    seq: i64,
) -> Result<u64, StoreError> {
    execute(
        connection,
        "INSERT INTO auth_log (seq, at, event, user_id, subject) VALUES (?1, ?2, ?3, ?4, ?5)",
        vec![
            int(seq),
            time(entry.at)?,
            json_enum(&entry.event)?,
            text(&entry.user),
            opt_text(entry.subject.as_ref()),
        ],
    )
    .await?;
    u64::try_from(seq).map_err(|error| corrupt(&format!("seq: {error}")))
}

/// The auth log below `horizon` (`crate::sequence`).
pub(crate) async fn auth_log(
    connection: &Connection,
    query: &AuthLogQuery,
    horizon: i64,
) -> Result<Page<LoggedAuthEntry, u64>, StoreError> {
    let after = query
        .after
        .map(|after| i64::try_from(after).unwrap_or(i64::MAX));
    let select = format!(
        "SELECT seq, at, event, user_id, subject FROM auth_log \
         WHERE (?1 IS NULL OR seq > ?1) AND (?2 IS NULL OR user_id = ?2) AND seq < ?3 \
         ORDER BY seq LIMIT {}",
        query.size.get() + 1
    );
    let params = vec![
        after.map_or(Value::Null, int),
        opt_text(query.user.as_ref()),
        int(horizon),
    ];
    let mut items = Vec::new();
    for row in rows(connection, &select, params).await? {
        items.push(LoggedAuthEntry {
            seq: u64::try_from(row.int(0)?).map_err(|error| corrupt(&format!("seq: {error}")))?,
            entry: AuthLogEntry {
                at: row.timestamp(1)?,
                event: enum_from(&row.text(2)?)?,
                user: row.parse(3)?,
                subject: row.opt_parse(4)?,
            },
        });
    }
    Ok(Page::cut(items, query.size, |logged| logged.seq))
}

pub(crate) async fn put_conversation(
    connection: &Connection,
    conversation: &ConversationRecord,
) -> Result<(), StoreError> {
    backend::check_conversation_size(conversation)?;
    execute(
        connection,
        "INSERT OR REPLACE INTO conversations (id, user_id, title, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        vec![
            text(&conversation.id),
            text(&conversation.user),
            opt_text(conversation.title.as_ref()),
            time(conversation.created_at)?,
            time(conversation.updated_at)?,
        ],
    )
    .await?;
    let sql = "DELETE FROM conversation_messages WHERE conversation = ?1";
    execute(connection, sql, vec![text(&conversation.id)]).await?;
    for (position, message) in (0_i64..).zip(&conversation.messages) {
        execute(
            connection,
            "INSERT INTO conversation_messages (conversation, position, author, at, content) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            vec![
                text(&conversation.id),
                int(position),
                json_enum(&message.author)?,
                time(message.at)?,
                text(&message.content),
            ],
        )
        .await?;
    }
    Ok(())
}

pub(crate) async fn conversation(
    connection: &Connection,
    id: &ConversationId,
) -> Result<Option<ConversationRecord>, StoreError> {
    let select = "SELECT user_id, title, created_at, updated_at FROM conversations WHERE id = ?1";
    let Some(row) = first(connection, select, vec![text(id)]).await? else {
        return Ok(None);
    };
    let select = "SELECT author, at, content FROM conversation_messages WHERE conversation = ?1 \
                  ORDER BY position";
    let mut messages = Vec::new();
    for message in rows(connection, select, vec![text(id)]).await? {
        messages.push(cairn_store::ConversationMessage {
            author: enum_from(&message.text(0)?)?,
            at: message.timestamp(1)?,
            content: message.parse(2)?,
        });
    }
    Ok(Some(ConversationRecord {
        id: id.clone(),
        user: row.parse(0)?,
        title: row.opt_parse(1)?,
        created_at: row.timestamp(2)?,
        updated_at: row.timestamp(3)?,
        messages,
    }))
}

pub(crate) async fn conversations_of(
    connection: &Connection,
    user: &UserId,
    after: Option<&ConversationId>,
    size: PageSize,
) -> Result<Page<ConversationSummary, ConversationId>, StoreError> {
    let select = format!(
        "SELECT id, title, updated_at FROM conversations \
         WHERE user_id = ?1 AND (?2 IS NULL OR id > ?2) ORDER BY id LIMIT {}",
        size.get() + 1
    );
    let mut items = Vec::new();
    for row in rows(connection, &select, vec![text(user), opt_text(after)]).await? {
        items.push(ConversationSummary {
            id: row.parse(0)?,
            title: row.opt_parse(1)?,
            updated_at: row.timestamp(2)?,
        });
    }
    Ok(Page::cut(items, size, |summary| summary.id.clone()))
}

pub(crate) async fn remove_conversation(
    connection: &Connection,
    id: &ConversationId,
) -> Result<bool, StoreError> {
    let sql = "DELETE FROM conversation_messages WHERE conversation = ?1";
    execute(connection, sql, vec![text(id)]).await?;
    let sql = "DELETE FROM conversations WHERE id = ?1";
    Ok(execute(connection, sql, vec![text(id)]).await? > 0)
}
