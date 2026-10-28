//! What a turn keeps of its tool traffic: every loop iteration resends the whole exchange, so
//! the results kept are bounded ([`keep_newest`]), and a call the model keeps repeating after
//! it failed is noticed ([`Failures`]).

use std::collections::BTreeMap;

use serde_json::Value;

use crate::limits::TOOL_CALL_FAILURE_REPEAT_COUNT_MAX;
use crate::provider::Message;

/// What a result dropped to fit the turn's limit says in its place.
pub const DROPPED: &str = "{\"dropped\":\"this result was left out to keep the turn's context \
                           small; read again what you still need\"}";

/// Keeps the newest tool results of `messages` whose content adds up to at most `bytes_max`
/// bytes, and replaces every older one with [`DROPPED`]. A result is kept whole or dropped
/// whole, and once one is dropped so is every result older than it.
pub fn keep_newest(messages: &mut [Message], bytes_max: u32) {
    let mut left = usize::try_from(bytes_max).unwrap_or(usize::MAX);
    let mut dropping = false;
    for message in messages.iter_mut().rev() {
        let Message::ToolResults(results) = message else {
            continue;
        };
        for result in results.iter_mut().rev() {
            if result.content == DROPPED {
                dropping = true;
            }
            if dropping || result.content.len() > left {
                dropping = true;
                DROPPED.clone_into(&mut result.content);
            } else {
                left -= result.content.len();
            }
        }
    }
}

/// The calls of a turn that failed, by tool and arguments.
#[derive(Debug, Default)]
pub struct Failures(BTreeMap<(String, String), u32>);

impl Failures {
    /// Notes that the call of `tool` with `arguments` failed; true once the same call has
    /// failed [`TOOL_CALL_FAILURE_REPEAT_COUNT_MAX`] times in the turn.
    pub fn failed(&mut self, tool: &str, arguments: &Value) -> bool {
        let times = self
            .0
            .entry((tool.to_owned(), arguments.to_string()))
            .or_default();
        *times += 1;
        *times >= TOOL_CALL_FAILURE_REPEAT_COUNT_MAX
    }
}
