//! An MCP client in process: rmcp's own client, its Streamable HTTP transport carried by a
//! client that hands each request to the API's router as served (auth layer, limits, and
//! all) instead of a socket, so a test drives `/api/mcp` exactly as a remote agent would.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HOST, WWW_AUTHENTICATE};
use axum::http::{HeaderName, HeaderValue, Method, Request, StatusCode};
use cairn_mcp::ToolSet;
use cairn_store::MemoryStore;
use http_body_util::BodyExt;
use rmcp::ServiceExt;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ClientJsonRpcMessage, GetPromptRequestParams,
    GetPromptResult, Prompt, ServerJsonRpcMessage, Tool,
};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::common::client_side_sse::BoxedSseResponse;
use rmcp::transport::streamable_http_client::{
    AuthRequiredError, StreamableHttpClient, StreamableHttpClientTransportConfig,
    StreamableHttpError, StreamableHttpPostResponse,
};
use serde_json::Value;
use tower::ServiceExt as _;

use super::World;

/// The URL the client is configured with; requests go to the router, not to this host.
const URL: &str = "http://cairn.test/api/mcp";

/// A failure inside the in-process client.
#[derive(Debug)]
pub struct ClientError(String);

impl std::fmt::Display for ClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ClientError {}

/// Streamable HTTP over the router: each POST is one request through the API as served.
#[derive(Clone)]
pub struct InProcess {
    router: Router,
    peer: SocketAddr,
}

type Answer = Result<StreamableHttpPostResponse, StreamableHttpError<ClientError>>;

impl InProcess {
    async fn post(
        &self,
        message: ClientJsonRpcMessage,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Answer {
        let body = serde_json::to_vec(&message).unwrap();
        let mut request = Request::builder()
            .method(Method::POST)
            .uri("/api/mcp")
            .header(HOST, "cairn.test")
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json, text/event-stream");
        if let Some(token) = auth_header {
            request = request.header(AUTHORIZATION, format!("Bearer {token}"));
        }
        for (name, value) in custom_headers {
            request = request.header(name, value);
        }
        let mut request = request.body(Body::from(body)).unwrap();
        request.extensions_mut().insert(ConnectInfo(self.peer));
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        answer(status, &headers, &bytes)
    }
}

/// What the transport makes of a response.
fn answer(status: StatusCode, headers: &axum::http::HeaderMap, bytes: &[u8]) -> Answer {
    if status == StatusCode::UNAUTHORIZED {
        let challenge = headers.get(WWW_AUTHENTICATE).and_then(|v| v.to_str().ok());
        let www_authenticate_header = challenge.unwrap_or_default().to_owned();
        return Err(StreamableHttpError::AuthRequired(AuthRequiredError::new(
            www_authenticate_header,
        )));
    }
    if matches!(status, StatusCode::ACCEPTED | StatusCode::NO_CONTENT) {
        return Ok(StreamableHttpPostResponse::Accepted);
    }
    let text = String::from_utf8_lossy(bytes).into_owned();
    if !status.is_success() {
        return Err(StreamableHttpError::UnexpectedServerResponse(
            format!("HTTP {status}: {text}").into(),
        ));
    }
    let message: ServerJsonRpcMessage = serde_json::from_slice(bytes)
        .map_err(|error| StreamableHttpError::Client(ClientError(format!("{error}: {text}"))))?;
    Ok(StreamableHttpPostResponse::Json(message, None))
}

impl StreamableHttpClient for InProcess {
    type Error = ClientError;

    async fn post_message(
        &self,
        _uri: Arc<str>,
        message: ClientJsonRpcMessage,
        _session_id: Option<Arc<str>>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Answer {
        self.post(message, auth_header, custom_headers).await
    }

    fn delete_session(
        &self,
        _uri: Arc<str>,
        _session_id: Arc<str>,
        _auth_header: Option<String>,
        _custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> impl Future<Output = Result<(), StreamableHttpError<ClientError>>> + Send + '_ {
        std::future::ready(Ok(()))
    }

    fn get_stream(
        &self,
        _uri: Arc<str>,
        _session_id: Option<Arc<str>>,
        _last_event_id: Option<String>,
        _auth_header: Option<String>,
        _custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> impl Future<Output = Result<BoxedSseResponse, StreamableHttpError<ClientError>>> + Send + '_
    {
        // The endpoint is stateless and answers in JSON: it opens no stream.
        std::future::ready(Err(StreamableHttpError::ServerDoesNotSupportSse))
    }
}

/// A connected agent: an MCP session through the API as `token` presents. It remembers
/// which tools it called, and checks every successful output against its tool's output
/// schema.
pub struct Agent {
    client: RunningService<RoleClient, ()>,
    outputs: BTreeMap<String, Value>,
    called: Mutex<BTreeSet<String>>,
}

impl Agent {
    /// Connects to `world`'s `/api/mcp` presenting `Bearer token`.
    pub async fn connect(world: &World, token: &str) -> Self {
        Self::try_connect(world, Some(token)).await.unwrap()
    }

    /// Connects, or the error initializing met.
    pub async fn try_connect(world: &World, token: Option<&str>) -> Result<Self, String> {
        let client = InProcess {
            router: world.router.clone(),
            peer: SocketAddr::from(([127, 0, 0, 1], 40_000)),
        };
        let mut config = StreamableHttpClientTransportConfig::with_uri(URL);
        if let Some(token) = token {
            config = config.auth_header(token);
        }
        let transport = StreamableHttpClientTransport::with_client(client, config);
        let client = ().serve(transport).await.map_err(|error| error.to_string())?;
        let definitions = ToolSet::<MemoryStore>::definitions().into_iter();
        let outputs = definitions.map(|tool| {
            let schema = Value::Object(tool.output_schema);
            (tool.name.to_owned(), schema)
        });
        Ok(Self {
            client,
            outputs: outputs.collect(),
            called: Mutex::default(),
        })
    }

    /// The tools this agent has called.
    pub fn called(&self) -> BTreeSet<String> {
        self.called.lock().unwrap().clone()
    }

    /// The instructions the server sent when the session began.
    pub fn instructions(&self) -> Option<String> {
        let info = self.client.peer_info()?;
        info.instructions.clone()
    }

    /// Every tool the server lists.
    pub async fn tools(&self) -> Vec<Tool> {
        self.client.list_all_tools().await.unwrap()
    }

    /// Every prompt the server lists.
    pub async fn prompts(&self) -> Vec<Prompt> {
        self.client.list_all_prompts().await.unwrap()
    }

    /// The prompt `name`.
    pub async fn prompt(&self, name: &str) -> Result<GetPromptResult, String> {
        let request = GetPromptRequestParams::new(name.to_owned());
        self.client
            .get_prompt(request)
            .await
            .map_err(|error| error.to_string())
    }

    /// Calls `name` with `arguments`: its result, or the protocol error.
    pub async fn call(&self, name: &str, arguments: Value) -> Result<CallToolResult, String> {
        let Value::Object(arguments) = arguments else {
            panic!("arguments are an object");
        };
        let request = CallToolRequestParams::new(name.to_owned()).with_arguments(arguments);
        self.called.lock().unwrap().insert(name.to_owned());
        self.client
            .call_tool(request)
            .await
            .map_err(|error| error.to_string())
    }

    /// Calls `name`, which must succeed: its structured output.
    pub async fn ok(&self, name: &str, arguments: Value) -> Value {
        let result = self.call(name, arguments).await.unwrap();
        let output = result.structured_content.unwrap_or(Value::Null);
        assert_ne!(result.is_error, Some(true), "{name}: {output}");
        let schema = &self.outputs[name];
        let validator = jsonschema::draft202012::new(schema).unwrap();
        let errors: Vec<String> = validator
            .iter_errors(&output)
            .map(|e| e.to_string())
            .collect();
        assert!(
            errors.is_empty(),
            "{name}'s output departs from its schema: {errors:#?}"
        );
        output
    }

    /// Calls `name`, which the tool must refuse: its structured error.
    pub async fn refused(&self, name: &str, arguments: Value) -> Value {
        let result = self.call(name, arguments).await.unwrap();
        let error = result.structured_content.unwrap_or(Value::Null);
        assert_eq!(result.is_error, Some(true), "{name}: {error}");
        error
    }
}
