//! A model provider for the assistant's tests: a loopback chat-completions server that plays
//! a script, so every turn runs the real HTTP client against it and the tests read what the
//! assistant sent.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde_json::{Value, json};

/// The key the assistant is configured with, which the server insists on.
pub const KEY: &str = "test-key";

/// One answer of the script.
#[derive(Clone, Debug)]
pub struct Step {
    /// The HTTP status; anything but 200 answers an error body that echoes the key.
    pub status: u16,
    /// How long the server takes to answer.
    pub delay: Duration,
    /// What the model says.
    pub say: Option<String>,
    /// The tools it calls, with their arguments.
    pub calls: Vec<(String, Value)>,
    /// Its `finish_reason`.
    pub finish: &'static str,
}

impl Step {
    fn new() -> Self {
        Self {
            status: 200,
            delay: Duration::ZERO,
            say: None,
            calls: Vec::new(),
            finish: "stop",
        }
    }

    /// A final answer in words.
    pub fn say(text: &str) -> Self {
        Self {
            say: Some(text.to_owned()),
            ..Self::new()
        }
    }

    /// A reply that calls one tool.
    pub fn call(name: &str, arguments: Value) -> Self {
        Self::calls([(name, arguments)])
    }

    /// A reply that calls several tools at once.
    pub fn calls<'a>(calls: impl IntoIterator<Item = (&'a str, Value)>) -> Self {
        Self {
            calls: calls
                .into_iter()
                .map(|(name, arguments)| (name.to_owned(), arguments))
                .collect(),
            ..Self::new()
        }
    }

    /// An HTTP error.
    pub fn status(status: u16) -> Self {
        Self {
            status,
            ..Self::new()
        }
    }

    /// The same step, answered `delay` late.
    pub fn after(self, delay: Duration) -> Self {
        Self { delay, ..self }
    }

    /// The same step, ended with `finish`.
    pub fn finishing(self, finish: &'static str) -> Self {
        Self { finish, ..self }
    }

    /// An answer that never comes in time.
    pub fn hang() -> Self {
        Self::say("late").after(Duration::from_hours(24))
    }
}

#[derive(Default)]
struct Played {
    script: VecDeque<Step>,
    requests: Vec<Value>,
}

/// The loopback server and what it has been sent.
pub struct Provider {
    url: String,
    played: Arc<Mutex<Played>>,
}

impl Provider {
    /// Serves on a loopback port, with nothing scripted yet.
    pub async fn start() -> Self {
        let played = Arc::new(Mutex::new(Played::default()));
        let router = Router::new()
            .route("/v1/chat/completions", post(complete))
            .with_state(Arc::clone(&played));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self {
            url: format!("http://{address}/v1"),
            played,
        }
    }

    /// The endpoint the assistant is configured with.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Appends `steps` to the script. With none left the model says it has nothing to add.
    pub fn script(&self, steps: impl IntoIterator<Item = Step>) {
        self.played.lock().unwrap().script.extend(steps);
    }

    /// The request bodies received, oldest first.
    pub fn requests(&self) -> Vec<Value> {
        self.played.lock().unwrap().requests.clone()
    }

    /// How many requests have arrived.
    pub fn request_count(&self) -> usize {
        self.played.lock().unwrap().requests.len()
    }
}

async fn complete(
    State(played): State<Arc<Mutex<Played>>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let authorized = headers
        .get("authorization")
        .and_then(|sent| sent.to_str().ok())
        == Some(&format!("Bearer {KEY}"));
    let step = {
        let mut played = played.lock().unwrap();
        played
            .requests
            .push(serde_json::from_str(&body).unwrap_or(Value::Null));
        played
            .script
            .pop_front()
            .unwrap_or_else(|| Step::say("Nothing more to add."))
    };
    if !authorized {
        return error(StatusCode::UNAUTHORIZED, "no key sent");
    }
    tokio::time::sleep(step.delay).await;
    if step.status != 200 {
        let status = StatusCode::from_u16(step.status).unwrap();
        return error(status, &format!("failed for key {KEY}"));
    }
    let calls: Vec<Value> = step
        .calls
        .iter()
        .enumerate()
        .map(|(index, (name, arguments))| {
            json!({ "id": format!("call_{index}"), "type": "function",
                "function": { "name": name, "arguments": arguments.to_string() } })
        })
        .collect();
    let mut message = json!({ "role": "assistant", "content": step.say });
    if !calls.is_empty() {
        message["tool_calls"] = Value::Array(calls);
    }
    let answer = json!({ "choices": [{ "message": message, "finish_reason": step.finish }],
        "usage": { "prompt_tokens": 100, "completion_tokens": 10,
            "prompt_tokens_details": { "cached_tokens": 60 } } });
    axum::Json(answer).into_response()
}

fn error(status: StatusCode, message: &str) -> Response {
    (
        status,
        axum::Json(json!({ "error": { "message": message } })),
    )
        .into_response()
}
