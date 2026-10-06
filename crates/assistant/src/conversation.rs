//! Conversations (ARCHITECTURE, Store trait: outside every domain, with no events): one per
//! target per user, under an id derived from both, so a user returning to a journey or a
//! route's draft picks up where they left off and no one else sees it. A conversation keeps
//! the user's words, the assistant's replies, and Cairn's report of each write the assistant
//! made; tool traffic is not kept, since every turn starts from a fresh read of its target.
//! It keeps its newest [`CONVERSATION_MESSAGE_COUNT_MAX`] messages (DECISIONS.md, 4.4).

use std::fmt;

use cairn_schema::{ConversationId, JourneyId, Markdown, RouteId, Timestamp, Title, UserId};
use cairn_store::{ConversationMessage, ConversationRecord, MessageAuthor};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::limits::CONVERSATION_MESSAGE_COUNT_MAX;
use crate::provider::{Message, Reply};

/// What a conversation is about (I5: the assistant reads a journey or a draft).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    /// A journey.
    Journey(JourneyId),
    /// A route's draft (A12), which may not be open yet, or the route not exist yet.
    RouteDraft(RouteId),
}

impl fmt::Display for Target {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Target::Journey(journey) => write!(formatter, "journey {journey}"),
            Target::RouteDraft(route) => write!(formatter, "the draft of route {route}"),
        }
    }
}

/// The conversation `user` holds about `target`: `cv_` and a digest of both.
#[must_use]
pub fn conversation_id(target: &Target, user: &UserId) -> ConversationId {
    let about = match target {
        Target::Journey(journey) => format!("journey {journey}"),
        Target::RouteDraft(route) => format!("route {route}"),
    };
    let digest = Sha256::digest(format!("{about}\n{user}").as_bytes());
    let hex = crate::hex(&digest[..16]);
    match format!("cv_{hex}").parse() {
        Ok(id) => id,
        Err(error) => unreachable!("`cv_` and hex digits is a conversation id: {error:?}"),
    }
}

/// A new, empty conversation.
#[must_use]
pub fn started(target: &Target, user: &UserId, now: Timestamp) -> ConversationRecord {
    let title = format!("Assistant: {target}");
    ConversationRecord {
        id: conversation_id(target, user),
        user: user.clone(),
        title: title
            .chars()
            .take(200)
            .collect::<String>()
            .parse::<Title>()
            .ok(),
        created_at: now,
        updated_at: now,
        messages: Vec::new(),
    }
}

/// `record` with `messages` appended at `now`, keeping its newest messages.
///
/// # Panics
///
/// Never: it asserts it kept no more than its limit.
pub fn append(record: &mut ConversationRecord, messages: Vec<ConversationMessage>, now: Timestamp) {
    record.messages.extend(messages);
    let keep = usize::try_from(CONVERSATION_MESSAGE_COUNT_MAX).unwrap_or(usize::MAX);
    let over = record.messages.len().saturating_sub(keep);
    record.messages.drain(..over);
    record.updated_at = now;
    assert!(
        record.messages.len() <= keep,
        "a conversation keeps at most its limit"
    );
}

/// A message by `author` at `at`, its text cut to a Markdown body's size; `None` for text
/// with nothing in it.
#[must_use]
pub fn message(author: MessageAuthor, at: Timestamp, text: &str) -> Option<ConversationMessage> {
    let limit = usize::try_from(cairn_schema::limits::BODY_BYTES_MAX).unwrap_or(usize::MAX);
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let content: Markdown = text[..end].parse().ok()?;
    Some(ConversationMessage {
        author,
        at,
        content,
    })
}

/// The conversation as a provider is sent it: the user's words as user messages, and the
/// assistant's replies with Cairn's reports of its writes as the assistant's, each run of
/// one side joined into one message. It starts with the user, as every protocol asks.
#[must_use]
pub fn dialogue(record: &ConversationRecord) -> Vec<Message> {
    let mut messages: Vec<Message> = Vec::new();
    for held in &record.messages {
        let text = held.content.as_str();
        let user = held.author == MessageAuthor::User;
        match messages.last_mut() {
            Some(Message::User { text: last }) if user => {
                last.push_str("\n\n");
                last.push_str(text);
            }
            Some(Message::Assistant(Reply {
                text: Some(last), ..
            })) if !user => {
                last.push_str("\n\n");
                last.push_str(text);
            }
            None if !user => {}
            _ if user => messages.push(Message::User {
                text: text.to_owned(),
            }),
            _ => messages.push(Message::Assistant(Reply {
                text: Some(text.to_owned()),
                ..Reply::default()
            })),
        }
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(minute: i64) -> Timestamp {
        Timestamp::from_second(minute * 60).unwrap()
    }

    fn record(messages: &[(MessageAuthor, &str)]) -> ConversationRecord {
        let target = Target::Journey("j_one".parse().unwrap());
        let mut record = started(&target, &"u_ann".parse().unwrap(), at(0));
        let messages = messages
            .iter()
            .enumerate()
            .filter_map(|(minute, (author, text))| {
                message(*author, at(i64::try_from(minute).unwrap()), text)
            })
            .collect();
        append(&mut record, messages, at(9));
        record
    }

    #[test]
    fn a_conversation_is_one_per_target_per_user() {
        let ann = "u_ann".parse().unwrap();
        let bob = "u_bob".parse().unwrap();
        let journey = Target::Journey("j_one".parse().unwrap());
        let route = Target::RouteDraft("one".parse().unwrap());
        let ids = [
            conversation_id(&journey, &ann),
            conversation_id(&journey, &bob),
            conversation_id(&route, &ann),
        ];
        assert_ne!(ids[0], ids[1], "two users on one target");
        assert_ne!(ids[0], ids[2], "one user on two targets");
        assert_eq!(ids[0], conversation_id(&journey, &ann), "stable");
    }

    #[test]
    fn the_dialogue_starts_with_the_user_and_alternates() {
        use MessageAuthor::{Assistant, Tool, User};
        type Case<'a> = (&'a [(MessageAuthor, &'a str)], &'a [&'a str]);
        let cases: [Case; 3] = [
            (
                &[
                    (User, "a"),
                    (Tool, "applied"),
                    (Assistant, "done"),
                    (User, "b"),
                ],
                &["user a", "assistant applied\n\ndone", "user b"],
            ),
            // A turn that ended without a reply leaves two user messages in a row.
            (&[(User, "a"), (User, "b")], &["user a\n\nb"]),
            // Trimming can leave the assistant first.
            (&[(Assistant, "x"), (User, "a")], &["user a"]),
        ];
        for (held, expected) in cases {
            let shown: Vec<String> = dialogue(&record(held))
                .into_iter()
                .map(|message| match message {
                    Message::User { text } => format!("user {text}"),
                    Message::Assistant(reply) => format!("assistant {}", reply.text.unwrap()),
                    Message::ToolResults(_) => unreachable!(),
                })
                .collect();
            assert_eq!(shown, expected, "{held:?}");
        }
    }

    #[test]
    fn a_conversation_keeps_its_newest_messages() {
        let limit = usize::try_from(CONVERSATION_MESSAGE_COUNT_MAX).unwrap();
        let mut record = record(&[]);
        let texts: Vec<String> = (0..=limit).map(|index| format!("m{index}")).collect();
        let messages = texts
            .iter()
            .filter_map(|text| message(MessageAuthor::User, at(1), text))
            .collect();
        append(&mut record, messages, at(2));
        assert_eq!(record.messages.len(), limit);
        assert_eq!(
            record.messages[0].content.as_str(),
            "m1",
            "the oldest went first"
        );
    }

    #[test]
    fn a_long_reply_is_cut_to_a_body_on_a_character_boundary() {
        let limit = usize::try_from(cairn_schema::limits::BODY_BYTES_MAX).unwrap();
        let long = "é".repeat(limit);
        let kept = message(MessageAuthor::Assistant, at(0), &long).unwrap();
        assert!(kept.content.as_str().len() <= limit);
        assert!(kept.content.as_str().len() > limit - 2);
        assert_eq!(message(MessageAuthor::Assistant, at(0), "  "), None);
    }
}
