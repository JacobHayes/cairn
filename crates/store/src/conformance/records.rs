//! Conformance cases for the records outside every domain: auth state (H1, H3, H4; 3.2
//! builds on them) and assistant conversations.

use std::collections::BTreeSet;

use cairn_schema::Timestamp;

use super::{Backend, open};
use crate::build::{at, id};
use crate::commit::StoreError;
use crate::query::PageSize;
use crate::records::{
    AgentTokenRecord, AuthEvent, AuthLogEntry, AuthLogQuery, ConversationMessage,
    ConversationRecord, IdentityRecord, MessageAuthor, OAuthStateKind, OAuthStateRecord,
    SecretHash, SessionRecord, UserRecord,
};
use crate::store::{AuthStore, ConversationStore};

/// A digest made of one repeated hex digit.
fn digest(digit: char) -> SecretHash {
    id(&digit.to_string().repeat(64))
}

fn user(key: &str) -> UserRecord {
    UserRecord {
        id: id(key),
        name: id("Someone"),
        created_at: at(0),
    }
}

fn identity(provider: &str, subject: &str, user: &str) -> IdentityRecord {
    IdentityRecord {
        provider: id(provider),
        subject: id(subject),
        user: id(user),
        verified_emails: BTreeSet::from([id("someone@example.org")]),
        linked_at: at(1),
    }
}

fn session(digit: char, user: &str, expires_at: Timestamp) -> SessionRecord {
    SessionRecord {
        token_hash: digest(digit),
        user: id(user),
        created_at: at(0),
        expires_at,
    }
}

fn token(agent: &str, digit: char, user: &str) -> AgentTokenRecord {
    AgentTokenRecord {
        agent: id(agent),
        token_hash: digest(digit),
        name: id("Script"),
        user: id(user),
        created_at: at(2),
        revoked_at: None,
    }
}

fn oauth(digit: char, expires_at: Timestamp) -> OAuthStateRecord {
    OAuthStateRecord {
        key_hash: digest(digit),
        kind: OAuthStateKind::PkceVerifier,
        payload: id(r#"{"verifier": "opaque"}"#),
        expires_at,
    }
}

/// Auth records are stored as written and load back equal; an identity, session, or
/// agent token naming a user that does not exist is not stored.
pub async fn auth_records_round_trip<B: Backend>(backend: &B) {
    let store = open(backend).await;
    let ann = user("u_ann");
    store.put_user(ann.clone()).await.unwrap();
    assert_eq!(store.user(&id("u_ann")).await.unwrap(), Some(ann));
    assert_eq!(store.user(&id("u_nobody")).await.unwrap(), None);

    let identities = [
        identity("oidc", "sub-1", "u_ann"),
        identity("tailscale", "ann@tailnet", "u_ann"),
    ];
    for held in &identities {
        store.put_identity(held.clone()).await.unwrap();
    }
    let found = store.identity(&id("oidc"), &id("sub-1")).await.unwrap();
    assert_eq!(found, Some(identities[0].clone()));
    assert_eq!(
        store.identities_of(&id("u_ann")).await.unwrap(),
        identities.to_vec()
    );
    assert!(
        store
            .remove_identity(&id("oidc"), &id("sub-1"))
            .await
            .unwrap()
    );
    assert!(
        !store
            .remove_identity(&id("oidc"), &id("sub-1"))
            .await
            .unwrap()
    );
    assert_eq!(
        store.identities_of(&id("u_ann")).await.unwrap(),
        identities[1..].to_vec()
    );

    let held = session('a', "u_ann", at(60));
    store.put_session(held.clone()).await.unwrap();
    assert_eq!(
        store.session(&digest('a'), at(30)).await.unwrap(),
        Some(held)
    );
    assert!(store.remove_session(&digest('a')).await.unwrap());
    assert_eq!(store.session(&digest('a'), at(30)).await.unwrap(), None);

    let orphans = [
        store
            .put_identity(identity("oidc", "sub-2", "u_nobody"))
            .await,
        store.put_session(session('b', "u_nobody", at(60))).await,
        store
            .put_agent_token(token("ag_orphan", 'c', "u_nobody"))
            .await,
    ];
    for orphan in orphans {
        assert!(
            matches!(orphan, Err(StoreError::Malformed(_))),
            "{orphan:?}"
        );
    }
}

/// Transient OAuth state is used once, and sessions and state are gone once expired.
pub async fn oauth_state_is_taken_once_and_expired_records_go<B: Backend>(backend: &B) {
    let store = open(backend).await;
    store.put_user(user("u_ann")).await.unwrap();
    store.put_oauth_state(oauth('1', at(10))).await.unwrap();
    store.put_oauth_state(oauth('2', at(10))).await.unwrap();
    let taken = store.take_oauth_state(&digest('1'), at(5)).await.unwrap();
    assert_eq!(taken, Some(oauth('1', at(10))));
    assert_eq!(
        store.take_oauth_state(&digest('1'), at(5)).await.unwrap(),
        None
    );
    assert_eq!(
        store.take_oauth_state(&digest('2'), at(10)).await.unwrap(),
        None
    );

    store.put_oauth_state(oauth('3', at(10))).await.unwrap();
    store
        .put_session(session('4', "u_ann", at(10)))
        .await
        .unwrap();
    store
        .put_session(session('5', "u_ann", at(20)))
        .await
        .unwrap();
    assert_eq!(store.session(&digest('4'), at(10)).await.unwrap(), None);
    assert_eq!(store.remove_expired(at(15)).await.unwrap(), 2);
    assert_eq!(
        store.session(&digest('5'), at(15)).await.unwrap(),
        Some(session('5', "u_ann", at(20)))
    );
}

/// Agent tokens are found by their digest alone, listed by user, and revoked by a put.
pub async fn agent_tokens_are_found_by_digest_and_revoked_by_a_put<B: Backend>(backend: &B) {
    let store = open(backend).await;
    store.put_user(user("u_ann")).await.unwrap();
    store.put_user(user("u_bob")).await.unwrap();
    let script = token("ag_script", 'd', "u_ann");
    store.put_agent_token(script.clone()).await.unwrap();
    store
        .put_agent_token(token("ag_other", 'e', "u_bob"))
        .await
        .unwrap();
    assert_eq!(
        store.agent_token(&digest('d')).await.unwrap(),
        Some(script.clone())
    );
    assert_eq!(store.agent_token(&digest('f')).await.unwrap(), None);
    assert_eq!(
        store.agent_tokens_of(&id("u_ann")).await.unwrap(),
        vec![script.clone()]
    );
    let revoked = AgentTokenRecord {
        revoked_at: Some(at(9)),
        ..script
    };
    store.put_agent_token(revoked.clone()).await.unwrap();
    assert_eq!(
        store.agent_token(&digest('d')).await.unwrap(),
        Some(revoked)
    );
    let clash = store.put_agent_token(token("ag_clash", 'd', "u_bob")).await;
    assert!(matches!(clash, Err(StoreError::Malformed(_))), "{clash:?}");
}

/// The auth log appends in order and pages, by user or not.
pub async fn the_auth_log_appends_in_order_and_pages_by_user<B: Backend>(backend: &B) {
    let store = open(backend).await;
    let entries = [
        ("u_ann", AuthEvent::UserCreated, None),
        ("u_ann", AuthEvent::IdentityLinked, Some("oidc sub-1")),
        ("u_bob", AuthEvent::UserCreated, None),
        ("u_ann", AuthEvent::TokenMinted, Some("ag_script")),
        ("u_ann", AuthEvent::TokenRevoked, Some("ag_script")),
    ];
    let mut positions = Vec::new();
    for (minute, (who, event, subject)) in (0..).zip(entries) {
        let entry = AuthLogEntry {
            at: at(minute),
            event,
            user: id(who),
            subject: subject.map(id),
        };
        positions.push(store.append_auth_log(entry).await.unwrap());
    }
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "{positions:?}"
    );
    let mut events = Vec::new();
    let mut after = None;
    loop {
        let query = AuthLogQuery {
            user: Some(id("u_ann")),
            after,
            size: PageSize::new(2),
        };
        let page = store.auth_log(&query).await.unwrap();
        events.extend(page.items.iter().map(|logged| logged.entry.event));
        match page.next {
            Some(next) => after = Some(next),
            None => break,
        }
    }
    assert_eq!(
        events,
        vec![
            AuthEvent::UserCreated,
            AuthEvent::IdentityLinked,
            AuthEvent::TokenMinted,
            AuthEvent::TokenRevoked,
        ]
    );
}

fn conversation(key: &str, user: &str, messages: Vec<ConversationMessage>) -> ConversationRecord {
    ConversationRecord {
        id: id(key),
        user: id(user),
        title: Some(id("Planning help")),
        created_at: at(0),
        updated_at: at(3),
        messages,
    }
}

fn message(author: MessageAuthor, minute: i64, text: &str) -> ConversationMessage {
    ConversationMessage {
        author,
        at: at(minute),
        content: id(text),
    }
}

/// Conversations are stored whole and load back equal, list by user, and are held to the
/// serialized size cap.
pub async fn conversations_round_trip_and_list_by_user<B: Backend>(backend: &B) {
    let store = open(backend).await;
    let talk = conversation(
        "cv_one",
        "u_ann",
        vec![
            message(MessageAuthor::User, 1, "What is left?"),
            message(MessageAuthor::Tool, 2, "{\"frontier\": []}"),
            message(MessageAuthor::Assistant, 3, "Nothing is left."),
        ],
    );
    store.put_conversation(talk.clone()).await.unwrap();
    store
        .put_conversation(conversation("cv_two", "u_ann", Vec::new()))
        .await
        .unwrap();
    store
        .put_conversation(conversation("cv_three", "u_bob", Vec::new()))
        .await
        .unwrap();
    assert_eq!(
        store.conversation(&id("cv_one")).await.unwrap(),
        Some(talk.clone())
    );
    let page = store
        .conversations_of(&id("u_ann"), None, PageSize::new(1))
        .await
        .unwrap();
    assert_eq!(page.items, vec![talk.summary()]);
    let rest = store
        .conversations_of(&id("u_ann"), page.next.as_ref(), PageSize::new(1))
        .await
        .unwrap();
    let ids: Vec<_> = rest
        .items
        .iter()
        .map(|summary| summary.id.as_str().to_owned())
        .collect();
    assert_eq!((ids, rest.next), (vec!["cv_two".to_owned()], None));
    assert!(store.remove_conversation(&id("cv_two")).await.unwrap());
    assert_eq!(store.conversation(&id("cv_two")).await.unwrap(), None);

    let long = (0..300)
        .map(|minute| message(MessageAuthor::Assistant, minute, &"x".repeat(60_000)))
        .collect();
    let error = store
        .put_conversation(conversation("cv_long", "u_ann", long))
        .await;
    assert!(matches!(error, Err(StoreError::Malformed(_))), "{error:?}");
}
