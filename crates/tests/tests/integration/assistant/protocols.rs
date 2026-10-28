//! The wire protocols (ARCHITECTURE, Assistant; I5: not tied to one vendor): each provider
//! round-trips a tool call against a recorded fixture of its wire format. A loopback server
//! stands in for the provider: it holds each request to the fixture's body and headers and
//! answers the fixture's response, so the HTTP path runs whole with no network and no real
//! credential.
#![cfg(test)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use cairn_assistant::limits::{PROVIDER_RETRY_COUNT_MAX, PROVIDER_RETRY_WAIT_MAX};
use cairn_assistant::protocol::{
    Credential, HttpProvider, ProviderConfig, anthropic, chat, openai, retry_after,
};
use cairn_assistant::{
    Exchange, Message, Protocol, Provider, ProviderError, Reply, Stop, ToolCall, ToolResult,
    ToolSpec, Usage,
};
use serde_json::{Value, json};

/// A protocol's fixture: where it posts, the headers it sends, and its exchanges in order.
struct Fixture {
    path: String,
    headers: Vec<(String, String)>,
    exchanges: VecDeque<(Value, Value)>,
}

fn fixture(name: &str) -> Fixture {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let headers = fixture["headers"].as_object().unwrap();
    let exchanges = fixture["exchanges"].as_array().unwrap();
    Fixture {
        path: fixture["path"].as_str().unwrap().to_owned(),
        headers: headers
            .iter()
            .map(|(name, value)| (name.clone(), value.as_str().unwrap().to_owned()))
            .collect(),
        exchanges: exchanges
            .iter()
            .map(|exchange| (exchange["request"].clone(), exchange["response"].clone()))
            .collect(),
    }
}

type Expected = Arc<Mutex<Fixture>>;

/// The stand-in provider: the next exchange's response when the request is the fixture's,
/// otherwise a 400 naming the difference.
async fn answer(
    State(expected): State<Expected>,
    headers: HeaderMap,
    body: String,
) -> (StatusCode, String) {
    let mut fixture = expected.lock().unwrap();
    for (name, value) in &fixture.headers {
        let sent = headers.get(name).and_then(|sent| sent.to_str().ok());
        if sent != Some(value.as_str()) {
            let error = json!({ "error": { "message": format!("header {name} differs (sent: {})", sent.is_some()) } });
            return (StatusCode::BAD_REQUEST, error.to_string());
        }
    }
    let Some((request, response)) = fixture.exchanges.pop_front() else {
        return (
            StatusCode::BAD_REQUEST,
            json!({ "error": "no exchange left" }).to_string(),
        );
    };
    let sent: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    if sent != request {
        let error = json!({ "error": { "message": format!("sent {sent}, expected {request}") } });
        return (StatusCode::BAD_REQUEST, error.to_string());
    }
    (StatusCode::OK, response.to_string())
}

/// Serves `fixture` on a loopback port; the base URL of its API.
async fn serve(fixture: Fixture) -> (String, Expected) {
    // Every fixture's API is under `/v1`, as its reference documents it.
    let base = "/v1";
    assert!(fixture.path.starts_with(base), "{}", fixture.path);
    let path = fixture.path.clone();
    let expected = Arc::new(Mutex::new(fixture));
    let router = Router::new()
        .route(&path, post(answer))
        .with_state(Arc::clone(&expected));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (format!("http://{address}{base}"), expected)
}

fn provider(protocol: Protocol, endpoint: &str, model: &str) -> HttpProvider {
    HttpProvider::new(ProviderConfig {
        protocol,
        endpoint: endpoint.parse().unwrap(),
        model: model.to_owned(),
        credential: Some(Credential::new("test-key".to_owned())),
    })
}

/// The exchange each fixture's first request was written from.
fn first_exchange() -> Exchange {
    let parameters = json!({ "type": "object", "properties": { "journey": { "type": "string",
        "description": "The journey." } }, "required": ["journey"] });
    Exchange {
        system: "You are a test assistant.".to_owned(),
        messages: vec![Message::User {
            text: "What is next on j_one?".to_owned(),
        }],
        tools: vec![ToolSpec {
            name: "get_snapshot".to_owned(),
            description: "Reads a journey.".to_owned(),
            parameters: parameters.as_object().unwrap().clone(),
        }],
    }
}

/// I5: each protocol sends the conversation and tools as its reference writes them, reads
/// the model's tool call, sends the tool's result back in its own shape (the reply's
/// reasoning unchanged where the protocol has any), and reads the final answer.
#[tokio::test]
async fn each_protocol_round_trips_a_tool_call_against_its_fixture() {
    let cases = [
        (Protocol::AnthropicMessages, "anthropic-messages"),
        (Protocol::OpenaiResponses, "openai-responses"),
        (Protocol::ChatCompletions, "chat-completions"),
    ];
    for (protocol, name) in cases {
        let fixture = fixture(name);
        let model = fixture.exchanges[0].0["model"].as_str().unwrap().to_owned();
        let (endpoint, expected) = serve(fixture).await;
        let provider = provider(protocol, &endpoint, &model);
        let mut exchange = first_exchange();
        let called = provider
            .send(&exchange)
            .await
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let [call] = called.calls.as_slice() else {
            panic!("{name}: {called:?}");
        };
        let ToolCall {
            id,
            name: tool,
            arguments,
        } = call;
        assert_eq!(
            (tool.as_str(), arguments),
            ("get_snapshot", &json!({ "journey": "j_one" })),
            "{name}"
        );
        let result = ToolResult {
            call_id: id.clone(),
            content: r#"{"revision":3}"#.to_owned(),
            is_error: false,
        };
        exchange.messages.push(Message::Assistant(called));
        exchange.messages.push(Message::ToolResults(vec![result]));
        let answered = provider
            .send(&exchange)
            .await
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(answered.calls.is_empty(), "{name}");
        assert_eq!(
            answered.text.as_deref(),
            Some("Nothing is blocked; start the plan."),
            "{name}"
        );
        assert!(
            expected.lock().unwrap().exchanges.is_empty(),
            "{name}: every exchange ran"
        );
    }
}

/// A provider's error is reported with its status and message, never the credential, and
/// the configuration's debug form hides the key.
#[tokio::test]
async fn a_provider_error_is_reported_without_the_credential() {
    let mut fixture = fixture("anthropic-messages");
    fixture.headers = vec![("x-api-key".to_owned(), "another-key".to_owned())];
    let model = fixture.exchanges[0].0["model"].as_str().unwrap().to_owned();
    let (endpoint, _) = serve(fixture).await;
    let provider = provider(Protocol::AnthropicMessages, &endpoint, &model);
    let failed = provider.send(&first_exchange()).await;
    let Err(ProviderError::Rejected { status, message }) = failed else {
        panic!("{failed:?}");
    };
    assert_eq!(status, 400, "{message}");
    assert!(!message.contains("test-key"), "{message}");
    let config = ProviderConfig {
        protocol: Protocol::ChatCompletions,
        endpoint: endpoint.parse().unwrap(),
        model,
        credential: Some(Credential::new("test-key".to_owned())),
    };
    assert!(!format!("{config:?}").contains("test-key"));
}

/// PRACTICES, Explicit limits: a provider answer past the response size is refused, not
/// read whole.
#[tokio::test]
async fn an_answer_past_the_response_size_is_refused() {
    let limit = cairn_assistant::limits::PROVIDER_RESPONSE_BYTES_MAX as usize;
    let router = Router::new().route(
        "/v1/messages",
        post(move || async move { "x".repeat(limit + 1) }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let provider = provider(
        Protocol::AnthropicMessages,
        &format!("http://{address}/v1"),
        "a-model",
    );
    let failed = provider.send(&first_exchange()).await;
    let Err(ProviderError::Failed { message }) = failed else {
        panic!("{failed:?}");
    };
    assert!(message.contains(&limit.to_string()), "{message}");
}

/// Every protocol's answer, usage and stop reason read into the same types: the cache's
/// tokens are counted apart from the input on all three, and a reply that did not finish says
/// why.
#[test]
fn usage_and_stop_reason_are_read_from_each_protocol() {
    let usage = |input, output, cache_read, cache_write| Usage {
        input,
        output,
        cache_read,
        cache_write,
    };
    let anthropic = |reason: &str| {
        let answer = json!({ "content": [], "stop_reason": reason, "usage": {
            "input_tokens": 10, "output_tokens": 20,
            "cache_read_input_tokens": 300, "cache_creation_input_tokens": 4000 } });
        anthropic::reply(&answer).unwrap()
    };
    let openai = |status: Value, details: Value| {
        let answer = json!({ "output": [], "status": status, "incomplete_details": details,
            "usage": { "input_tokens": 310, "output_tokens": 20,
                "input_tokens_details": { "cached_tokens": 300 } } });
        openai::reply(&answer).unwrap()
    };
    let chat = |reason: &str| {
        let answer = json!({ "choices": [{ "message": { "content": "hi" },
            "finish_reason": reason }],
            "usage": { "prompt_tokens": 4310, "completion_tokens": 20, "prompt_tokens_details": {
                "cached_tokens": 300, "cache_write_tokens": 4000 } } });
        chat::reply(&answer).unwrap()
    };
    let same = usage(10, 20, 300, 4000);
    let summaries = |replies: Vec<(Reply, Stop)>| -> Vec<(Usage, Stop)> {
        replies
            .into_iter()
            .map(|(reply, stop)| {
                assert_eq!(reply.stop, stop);
                (reply.usage, reply.stop)
            })
            .collect()
    };
    let read = summaries(vec![
        (anthropic("end_turn"), Stop::Done),
        (anthropic("tool_use"), Stop::Done),
        (anthropic("max_tokens"), Stop::OutputLimit),
        (anthropic("refusal"), Stop::Refused),
        (anthropic("pause_turn"), Stop::Paused),
        (chat("stop"), Stop::Done),
        (chat("tool_calls"), Stop::Done),
        (chat("length"), Stop::OutputLimit),
        (chat("content_filter"), Stop::Refused),
    ]);
    assert!(read.iter().all(|(read, _)| *read == same), "{read:?}");
    let completed = openai(json!("completed"), Value::Null);
    let cut = openai(
        json!("incomplete"),
        json!({ "reason": "max_output_tokens" }),
    );
    let filtered = openai(json!("incomplete"), json!({ "reason": "content_filter" }));
    assert_eq!(completed.usage, usage(10, 20, 300, 0));
    assert_eq!(
        [completed.stop, cut.stop, filtered.stop],
        [Stop::Done, Stop::OutputLimit, Stop::Refused]
    );
}

/// A refusal part in a Responses answer is a refusal, though the response completed.
#[test]
fn a_responses_refusal_part_is_a_refusal() {
    let answer = json!({ "status": "completed", "output": [{ "type": "message", "content": [
        { "type": "refusal", "refusal": "I cannot help with that." }] }] });
    assert_eq!(openai::reply(&answer).unwrap().stop, Stop::Refused);
}

/// A model that does not reason refuses being asked for encrypted reasoning: the call is
/// sent again at once without it, and later calls do not ask.
#[tokio::test]
async fn a_model_that_refuses_encrypted_reasoning_is_not_asked_again() {
    let seen: Arc<Mutex<Vec<bool>>> = Arc::default();
    let handler = |State(seen): State<Arc<Mutex<Vec<bool>>>>, body: String| async move {
        let body: Value = serde_json::from_str(&body).unwrap();
        let asked = body.get("include").is_some();
        seen.lock().unwrap().push(asked);
        if asked {
            let error = json!({ "error": { "message": "Encrypted content is not supported with this model." } });
            return (StatusCode::BAD_REQUEST, error.to_string());
        }
        let answer = json!({ "status": "completed", "output": [{ "type": "message",
            "content": [{ "type": "output_text", "text": "hi" }] }] });
        (StatusCode::OK, answer.to_string())
    };
    let router = Router::new()
        .route("/v1/responses", post(handler))
        .with_state(Arc::clone(&seen));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let provider = provider(
        Protocol::OpenaiResponses,
        &format!("http://{address}/v1"),
        "a-model",
    );
    for _ in 0..2 {
        let answered = provider.send(&first_exchange()).await.unwrap();
        assert_eq!(answered.text.as_deref(), Some("hi"));
    }
    assert_eq!(*seen.lock().unwrap(), [true, false, false]);
}

/// What a stand-in provider answers: a status, `Retry-After` seconds, and a body.
type Answer = (u16, Option<u64>, String);

/// The stand-in's state: the answers, and the count of requests seen.
type Flaky = (Arc<Vec<Answer>>, Arc<Mutex<usize>>);

/// A stand-in provider that answers each request with the next of `answers` (status,
/// `Retry-After` seconds, body), the last forever, and counts the requests.
async fn flaky(answers: Vec<Answer>) -> (String, Arc<Mutex<usize>>) {
    let seen = Arc::new(Mutex::new(0_usize));
    let state: Flaky = (Arc::new(answers), Arc::clone(&seen));
    let handler = |State((answers, seen)): State<Flaky>| async move {
        let index = {
            let mut seen = seen.lock().unwrap();
            *seen += 1;
            *seen - 1
        };
        let (status, wait, body) = answers[index.min(answers.len() - 1)].clone();
        let mut headers = HeaderMap::new();
        if let Some(wait) = wait {
            headers.insert("retry-after", wait.to_string().parse().unwrap());
        }
        (StatusCode::from_u16(status).unwrap(), headers, body)
    };
    let router = Router::new()
        .route("/v1/messages", post(handler))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (format!("http://{address}/v1"), seen)
}

fn ok_body() -> String {
    json!({ "content": [{ "type": "text", "text": "hi" }], "stop_reason": "end_turn" }).to_string()
}

fn error_body() -> String {
    json!({ "error": { "message": "try later" } }).to_string()
}

/// A rate limit or a server error (529 included) is retried, a bad request is not, and the
/// retries are bounded.
#[tokio::test(start_paused = true)]
async fn rate_limits_and_server_errors_are_retried_a_bounded_number_of_times() {
    let retried = [429_u16, 500, 502, 529];
    for status in retried {
        let (endpoint, seen) =
            flaky(vec![(status, None, error_body()), (200, None, ok_body())]).await;
        let client = provider(Protocol::AnthropicMessages, &endpoint, "m");
        let answered = client.send(&first_exchange()).await;
        assert!(answered.is_ok(), "{status}: {answered:?}");
        assert_eq!(*seen.lock().unwrap(), 2, "{status}");
    }
    // Not worth retrying: the request itself is wrong, or the key is.
    for status in [400_u16, 401, 404] {
        let (endpoint, seen) = flaky(vec![(status, None, error_body())]).await;
        let client = provider(Protocol::AnthropicMessages, &endpoint, "m");
        let failed = client.send(&first_exchange()).await;
        assert!(
            matches!(failed, Err(ProviderError::Rejected { status: got, .. }) if got == status),
            "{status}: {failed:?}"
        );
        assert_eq!(*seen.lock().unwrap(), 1, "{status}");
    }
    // Always failing: the first call and its bounded retries, then the last error.
    let (endpoint, seen) = flaky(vec![(503, None, "<html>bad gateway</html>".to_owned())]).await;
    let client = provider(Protocol::AnthropicMessages, &endpoint, "m");
    let failed = client.send(&first_exchange()).await;
    assert!(
        matches!(failed, Err(ProviderError::Rejected { status: 503, .. })),
        "{failed:?}"
    );
    assert_eq!(*seen.lock().unwrap(), 1 + PROVIDER_RETRY_COUNT_MAX as usize);
}

/// A provider's `Retry-After` is waited out before the retry; one asking for more than a
/// turn should wait fails the call at once.
#[tokio::test(start_paused = true)]
async fn retry_after_is_honoured_up_to_a_limit() {
    let wait = PROVIDER_RETRY_WAIT_MAX.as_secs() - 1;
    let (endpoint, _) = flaky(vec![
        (429, Some(wait), error_body()),
        (200, None, ok_body()),
    ])
    .await;
    let client = provider(Protocol::AnthropicMessages, &endpoint, "m");
    let started = tokio::time::Instant::now();
    client.send(&first_exchange()).await.unwrap();
    assert!(
        started.elapsed().as_secs() >= wait,
        "{:?}",
        started.elapsed()
    );

    let too_long = PROVIDER_RETRY_WAIT_MAX.as_secs() + 1;
    let (endpoint, seen) = flaky(vec![
        (429, Some(too_long), error_body()),
        (200, None, ok_body()),
    ])
    .await;
    let client = provider(Protocol::AnthropicMessages, &endpoint, "m");
    assert!(client.send(&first_exchange()).await.is_err());
    assert_eq!(*seen.lock().unwrap(), 1);
}

/// `Retry-After` is read in both forms HTTP gives it, seconds or a date, and a date already
/// past asks for no wait.
#[test]
fn retry_after_is_read_as_seconds_or_as_a_date() {
    let now: jiff::Timestamp = "2026-10-28T12:00:00Z".parse().unwrap();
    let read = |value: &str| retry_after(value, now);
    assert_eq!(read(" 7 "), Some(Duration::from_secs(7)));
    assert_eq!(
        read("Wed, 28 Oct 2026 12:00:29 GMT"),
        Some(Duration::from_secs(29))
    );
    assert_eq!(read("Wed, 28 Oct 2026 11:00:00 GMT"), Some(Duration::ZERO));
    assert_eq!(read("soon"), None);
}

/// An error that echoes the key is reported without it.
#[tokio::test]
async fn an_error_echoing_the_key_is_redacted() {
    let body = json!({ "error": { "message": "bad key test-key" } }).to_string();
    let (endpoint, _) = flaky(vec![(401, None, body)]).await;
    let client = provider(Protocol::AnthropicMessages, &endpoint, "m");
    let failed = client.send(&first_exchange()).await;
    let Err(ProviderError::Rejected { message, .. }) = failed else {
        panic!("{failed:?}");
    };
    assert!(!message.contains("test-key"), "{message}");
}
