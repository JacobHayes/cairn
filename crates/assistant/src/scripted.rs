//! A scripted provider: it answers each call with the next step of a script and records what
//! it was sent, so the turn loop, the write wrapper, and the endpoints are tested, and the
//! proof is generated, with no network and no credential.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;

use crate::provider::{Answer, Exchange, Provider, ProviderError, Reply, ToolCall};

/// One scripted answer.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Answer this reply.
    Reply(Reply),
    /// Fail with this error.
    Fail(ProviderError),
    /// Never answer, as a provider that is down: the turn's limit cuts the call off.
    Hang,
    /// Answer this reply after this long, as a slow provider.
    Slow(Duration, Reply),
}

impl Step {
    /// A reply that says `text` and ends the turn.
    #[must_use]
    pub fn say(text: &str) -> Self {
        Step::Reply(Reply {
            text: Some(text.to_owned()),
            ..Reply::default()
        })
    }

    /// A reply that calls one tool.
    #[must_use]
    pub fn call(name: &str, arguments: Value) -> Self {
        Self::calls([(name, arguments)])
    }

    /// A reply that calls several tools, in order; their ids are `call_0` and on.
    #[must_use]
    pub fn calls<'a>(calls: impl IntoIterator<Item = (&'a str, Value)>) -> Self {
        let calls = calls
            .into_iter()
            .enumerate()
            .map(|(index, (name, arguments))| ToolCall {
                id: format!("call_{index}"),
                name: name.to_owned(),
                arguments,
            })
            .collect();
        Step::Reply(Reply {
            calls,
            ..Reply::default()
        })
    }
}

/// A provider that plays a script.
#[derive(Debug, Default)]
pub struct ScriptedProvider {
    steps: Mutex<VecDeque<Step>>,
    seen: Mutex<Vec<Exchange>>,
}

impl ScriptedProvider {
    /// A provider that answers `steps` in order, then fails.
    #[must_use]
    pub fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: Mutex::new(steps.into_iter().collect()),
            seen: Mutex::new(Vec::new()),
        }
    }

    /// Appends `steps` to the script.
    pub fn push(&self, steps: impl IntoIterator<Item = Step>) {
        lock(&self.steps).extend(steps);
    }

    /// Every exchange it was sent, in order.
    #[must_use]
    pub fn exchanges(&self) -> Vec<Exchange> {
        lock(&self.seen).clone()
    }

    /// Steps not yet played.
    #[must_use]
    pub fn remaining(&self) -> usize {
        lock(&self.steps).len()
    }
}

impl Provider for ScriptedProvider {
    fn send<'a>(&'a self, exchange: &'a Exchange) -> Answer<'a> {
        lock(&self.seen).push(exchange.clone());
        let step = lock(&self.steps).pop_front();
        Box::pin(async move {
            match step {
                Some(Step::Reply(reply)) => Ok(reply),
                Some(Step::Fail(error)) => Err(error),
                Some(Step::Hang) => std::future::pending().await,
                Some(Step::Slow(after, reply)) => {
                    tokio::time::sleep(after).await;
                    Ok(reply)
                }
                None => Err(ProviderError::Failed {
                    message: "the script has ended".to_owned(),
                }),
            }
        })
    }
}

/// The value behind `mutex`, whether or not a holder panicked: the script is plain data.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
