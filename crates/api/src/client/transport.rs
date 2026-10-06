//! The client's HTTP transport: one HTTP/1.1 connection per request to the server's socket
//! address, over tokio, with an optional observer that sees every exchange (the proof's
//! transcripts). Plain HTTP to a known address is what the tests and the multiplayer
//! testbed need; TLS and name resolution wait for a client that leaves the machine.

use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Bytes;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE, HOST};
use axum::http::{HeaderMap, HeaderValue, Method, Request, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
use serde::de::DeserializeOwned;

/// One request and its response, as an observer sees it.
#[derive(Clone, Debug)]
pub struct Exchange {
    /// The method.
    pub method: Method,
    /// The path and query.
    pub target: String,
    /// The headers sent.
    pub request_headers: HeaderMap,
    /// The body sent.
    pub request_body: Bytes,
    /// The status answered.
    pub status: StatusCode,
    /// The headers answered.
    pub response_headers: HeaderMap,
    /// The body answered; for a stream, what was read of it.
    pub response_body: Bytes,
}

/// What sees each exchange.
pub type Observer = Arc<dyn Fn(&Exchange) + Send + Sync>;

/// A response read whole.
#[derive(Clone, Debug)]
pub struct Reply {
    /// The status.
    pub status: StatusCode,
    /// The headers.
    pub headers: HeaderMap,
    /// The body.
    pub body: Bytes,
}

impl Reply {
    /// The body as JSON of `T`.
    ///
    /// # Errors
    ///
    /// When it does not parse as `T`.
    pub fn json<T: DeserializeOwned>(&self) -> Result<T, TransportError> {
        serde_json::from_slice(&self.body).map_err(|error| {
            let body = String::from_utf8_lossy(&self.body);
            TransportError(format!(
                "a {} body that is not what was expected ({error}): {body}",
                self.status
            ))
        })
    }
}

/// The request could not be sent or its response read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportError(pub String);

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for TransportError {}

fn failed(what: &str) -> impl Fn(hyper::Error) -> TransportError + '_ {
    move |error| TransportError(format!("{what}: {error}"))
}

/// HTTP to one server as one caller.
#[derive(Clone)]
pub struct Transport {
    address: SocketAddr,
    token: Option<String>,
    observer: Option<Observer>,
}

impl fmt::Debug for Transport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Transport")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

impl Transport {
    /// To the server at `address`, presenting `token` as a bearer token when given.
    #[must_use]
    pub fn new(address: SocketAddr, token: Option<String>) -> Self {
        Self {
            address,
            token,
            observer: None,
        }
    }

    /// The same, with `observer` seeing every exchange.
    #[must_use]
    pub fn observed(mut self, observer: Observer) -> Self {
        self.observer = Some(observer);
        self
    }

    /// The server's address.
    #[must_use]
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Sends `method` to `target` (path and query) with a JSON `body`, if any, and reads the
    /// whole response.
    ///
    /// # Errors
    ///
    /// When the server cannot be reached or the response not read.
    pub async fn send(
        &self,
        method: Method,
        target: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<Reply, TransportError> {
        let bytes = match body {
            Some(value) => Bytes::from(serde_json::to_vec(value).map_err(|error| {
                TransportError(format!("the body does not serialize: {error}"))
            })?),
            None => Bytes::new(),
        };
        let content_type = body.map(|_| HeaderValue::from_static("application/json"));
        self.send_raw(method, target, content_type, bytes).await
    }

    /// Sends a body as it is, with `content_type` if given, and reads the whole response.
    ///
    /// # Errors
    ///
    /// When the server cannot be reached or the response not read.
    pub async fn send_raw(
        &self,
        method: Method,
        target: &str,
        content_type: Option<HeaderValue>,
        body: Bytes,
    ) -> Result<Reply, TransportError> {
        let request = self.request(method, target, content_type, body.clone())?;
        let (method, request_headers) = (request.method().clone(), request.headers().clone());
        let response = self.open(request).await?;
        let (parts, incoming) = response.into_parts();
        let collected = incoming
            .collect()
            .await
            .map_err(failed("reading the body"))?;
        let reply = Reply {
            status: parts.status,
            headers: parts.headers,
            body: collected.to_bytes(),
        };
        self.observe(&Exchange {
            method,
            target: target.to_owned(),
            request_headers,
            request_body: body,
            status: reply.status,
            response_headers: reply.headers.clone(),
            response_body: reply.body.clone(),
        });
        Ok(reply)
    }

    /// Shows an exchange to the observer, if there is one.
    pub fn observe(&self, exchange: &Exchange) {
        if let Some(observer) = &self.observer {
            observer(exchange);
        }
    }

    /// A request to `target` with the caller's credentials.
    ///
    /// # Errors
    ///
    /// When the target or token cannot be a request.
    pub fn request(
        &self,
        method: Method,
        target: &str,
        content_type: Option<HeaderValue>,
        body: Bytes,
    ) -> Result<Request<Full<Bytes>>, TransportError> {
        let mut builder = Request::builder()
            .method(method)
            .uri(target)
            .header(HOST, self.address.to_string());
        if let Some(token) = &self.token {
            builder = builder.header(AUTHORIZATION, format!("Bearer {token}"));
        }
        if let Some(content_type) = content_type {
            builder = builder.header(CONTENT_TYPE, content_type);
        }
        builder
            .body(Full::new(body))
            .map_err(|error| TransportError(format!("not a request: {error}")))
    }

    /// Sends `request` on a fresh connection and answers its response head; the body
    /// arrives as the caller reads it.
    ///
    /// # Errors
    ///
    /// When the server cannot be reached.
    pub async fn open(
        &self,
        request: Request<Full<Bytes>>,
    ) -> Result<hyper::Response<Incoming>, TransportError> {
        let stream = tokio::net::TcpStream::connect(self.address)
            .await
            .map_err(|error| TransportError(format!("connecting to {}: {error}", self.address)))?;
        let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .map_err(failed("the handshake"))?;
        tokio::spawn(async move {
            // The connection ends when the response is read or dropped; an error then is
            // the caller's to see through the response, not the driver's.
            let _ = connection.await;
        });
        sender
            .send_request(request)
            .await
            .map_err(failed("sending the request"))
    }
}
