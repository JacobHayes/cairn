//! Conversations (ARCHITECTURE, Store trait: outside every domain, with no events): one per
//! target per user, under an id derived from both, so a user returning to a journey or a
//! route's draft picks up where they left off and no one else sees it. A conversation keeps
//! the user's words, the assistant's replies, and a plain line for each write the assistant
//! made; tool traffic is not kept, since the assistant reads its target through tools each turn.
//! It keeps its newest [`CONVERSATION_MESSAGE_COUNT_MAX`] messages
//! (decisions/2026-10-06-conversations-are-one-per-target-per-user-keep-their-newest.md).

use std::fmt;

use cairn_schema::{ConversationId, JourneyId, Markdown, RouteId, Timestamp, Title, UserId};
use cairn_store::{ConversationMessage, ConversationRecord, MessageAuthor};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::limits::CONVERSATION_MESSAGE_COUNT_MAX;

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
/// assistant's replies with the lines about its writes as the assistant's, each run of one
/// side joined into one message. It holds the newest messages within `bytes_max` bytes of
/// text, and starts with the user.
#[must_use]
pub fn dialogue(record: &ConversationRecord, bytes_max: u32) -> Vec<Value> {
    let budget = usize::try_from(bytes_max).unwrap_or(usize::MAX);
    let mut spent = 0_usize;
    let kept = record
        .messages
        .iter()
        .rev()
        .take_while(|held| {
            spent = spent.saturating_add(held.content.as_str().len());
            spent <= budget
        })
        .count();
    let newest = &record.messages[record.messages.len() - kept..];
    let mut runs: Vec<(bool, String)> = Vec::new();
    for held in newest {
        let user = held.author == MessageAuthor::User;
        match runs.last_mut() {
            Some((side, text)) if *side == user => {
                text.push_str("\n\n");
                text.push_str(held.content.as_str());
            }
            // Trimming can leave the assistant first.
            None if !user => {}
            _ => runs.push((user, held.content.as_str().to_owned())),
        }
    }
    runs.into_iter()
        .map(|(user, text)| json!({ "role": if user { "user" } else { "assistant" }, "content": text }))
        .collect()
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

    /// The roles and texts of a dialogue.
    fn shown(held: &ConversationRecord, budget: u32) -> Vec<(String, String)> {
        dialogue(held, budget)
            .into_iter()
            .map(|message| {
                let field = |name: &str| message[name].as_str().unwrap().to_owned();
                (field("role"), field("content"))
            })
            .collect()
    }

    #[test]
    fn the_dialogue_starts_with_the_user_and_alternates() {
        use MessageAuthor::{Assistant, Tool, User};
        type Case<'a> = (&'a [(MessageAuthor, &'a str)], &'a [(&'a str, &'a str)]);
        let cases: [Case; 3] = [
            (
                &[
                    (User, "a"),
                    (Tool, "applied"),
                    (Assistant, "done"),
                    (User, "b"),
                ],
                &[
                    ("user", "a"),
                    ("assistant", "applied\n\ndone"),
                    ("user", "b"),
                ],
            ),
            // A turn that ended without a reply leaves two user messages in a row.
            (&[(User, "a"), (User, "b")], &[("user", "a\n\nb")]),
            // Trimming can leave the assistant first.
            (&[(Assistant, "x"), (User, "a")], &[("user", "a")]),
        ];
        for (held, expected) in cases {
            let expected: Vec<(String, String)> = expected
                .iter()
                .map(|(role, text)| ((*role).to_owned(), (*text).to_owned()))
                .collect();
            assert_eq!(shown(&record(held), u32::MAX), expected, "{held:?}");
        }
    }

    #[test]
    fn the_dialogue_replays_the_newest_messages_within_its_budget() {
        use MessageAuthor::{Assistant, User};
        let held = record(&[
            (User, "aaaa"),
            (Assistant, "bbbb"),
            (User, "cccc"),
            (Assistant, "dd"),
        ]);
        let cases = [(u32::MAX, 4), (10, 2), (6, 2), (2, 0)];
        for (budget, count) in cases {
            let replayed = shown(&held, budget);
            let bytes: usize = replayed.iter().map(|(_, text)| text.len()).sum();
            assert!(bytes <= budget as usize, "{budget}");
            assert_eq!(replayed.len(), count, "{budget}: {replayed:?}");
            assert!(replayed.first().is_none_or(|(role, _)| role == "user"));
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

/// One kept message of a conversation, as the API shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConversationEntry {
    /// Who wrote it.
    pub author: ConversationAuthor,
    /// When it was kept.
    pub at: Timestamp,
    /// What it says.
    pub content: Markdown,
}

/// Who wrote a conversation message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConversationAuthor {
    /// The user.
    User,
    /// The assistant.
    Assistant,
    /// Cairn, reporting a write the assistant made or how a turn ended.
    Cairn,
}

impl From<ConversationMessage> for ConversationEntry {
    fn from(message: ConversationMessage) -> Self {
        let ConversationMessage {
            author,
            at,
            content,
        } = message;
        let author = match author {
            MessageAuthor::User => ConversationAuthor::User,
            MessageAuthor::Assistant => ConversationAuthor::Assistant,
            MessageAuthor::Tool => ConversationAuthor::Cairn,
        };
        Self {
            author,
            at,
            content,
        }
    }
}
