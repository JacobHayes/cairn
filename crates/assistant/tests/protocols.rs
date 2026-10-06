//! The wire protocols (ARCHITECTURE, Assistant; I5: not tied to one vendor): each provider
//! round-trips a tool call against a recorded fixture of its wire format. A loopback server
//! stands in for the provider: it holds each request to the fixture's body and headers and
//! answers the fixture's response, so the HTTP path runs whole with no network and no real
//! credential.
#![cfg(test)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use cairn_assistant::protocol::{Credential, HttpProvider, ProviderConfig};
use cairn_assistant::{
    Exchange, Message, Protocol, Provider, ProviderError, ToolCall, ToolResult, ToolSpec,
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
    let Err(ProviderError::Failed { message }) = failed else {
        panic!("{failed:?}");
    };
    assert!(message.contains("400"), "{message}");
    assert!(!message.contains("test-key"), "{message}");
    let config = ProviderConfig {
        protocol: Protocol::ChatCompletions,
        endpoint: endpoint.parse().unwrap(),
        model,
        credential: Some(Credential::new("test-key".to_owned())),
    };
    assert!(!format!("{config:?}").contains("test-key"));
}
