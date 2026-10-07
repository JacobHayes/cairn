//! Reading a server-sent event stream as it arrives (the SSE wire format: blocks of
//! `field: value` lines ended by a blank line; a line starting with `:` is a comment).

use axum::body::Bytes;
use axum::http::{Method, StatusCode};
use http_body_util::BodyExt;
use hyper::body::Incoming;

use super::transport::{Reply, Transport, TransportError};

/// One event as it arrived.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SseEvent {
    /// Its `event` name, if it had one.
    pub event: Option<String>,
    /// Its `data` lines, joined by newlines.
    pub data: String,
    /// Its comment lines, joined by newlines: a heartbeat is a block with only a comment.
    pub comment: String,
}

/// What opening a stream answered.
#[derive(Debug)]
pub enum Opened {
    /// The stream, to read as it arrives.
    Stream(Box<EventStream>),
    /// A refusal, read whole.
    Refused(Reply),
}

/// A stream being read. Dropping it closes the connection.
#[derive(Debug)]
pub struct EventStream {
    body: Incoming,
    pending: Vec<u8>,
}

impl Transport {
    /// GETs the SSE stream at `target`.
    ///
    /// # Errors
    ///
    /// When the server cannot be reached, or a refusal not read.
    pub async fn stream(&self, target: &str) -> Result<Opened, TransportError> {
        let request = self.request(Method::GET, target, None, Bytes::new())?;
        let response = self.open(request).await?;
        let (parts, body) = response.into_parts();
        if parts.status != StatusCode::OK {
            let collected = body
                .collect()
                .await
                .map_err(|error| TransportError(format!("reading the refusal: {error}")))?;
            return Ok(Opened::Refused(Reply {
                status: parts.status,
                headers: parts.headers,
                body: collected.to_bytes(),
            }));
        }
        Ok(Opened::Stream(Box::new(EventStream {
            body,
            pending: Vec::new(),
        })))
    }
}

impl EventStream {
    /// The next event; none when the stream has ended.
    ///
    /// # Errors
    ///
    /// When the connection fails or the server ends the stream with an error.
    pub async fn next_event(&mut self) -> Result<Option<SseEvent>, TransportError> {
        loop {
            if let Some(event) = self.take_block() {
                return Ok(Some(event));
            }
            let Some(frame) = self.body.frame().await else {
                return Ok(None);
            };
            let frame = frame.map_err(|error| TransportError(format!("the stream: {error}")))?;
            if let Ok(data) = frame.into_data() {
                self.pending
                    .extend(data.iter().copied().filter(|byte| *byte != b'\r'));
            }
        }
    }

    /// The first complete block in what has arrived, parsed.
    fn take_block(&mut self) -> Option<SseEvent> {
        let end = self.pending.windows(2).position(|pair| pair == b"\n\n")?;
        let block: Vec<u8> = self.pending.drain(..end + 2).collect();
        let text = String::from_utf8_lossy(&block);
        let mut event = SseEvent::default();
        let mut data = Vec::new();
        let mut comments = Vec::new();
        for line in text.lines().filter(|line| !line.is_empty()) {
            if let Some(comment) = line.strip_prefix(':') {
                comments.push(comment.trim_start().to_owned());
                continue;
            }
            let (field, value) = line.split_once(':').unwrap_or((line, ""));
            let value = value.strip_prefix(' ').unwrap_or(value);
            match field {
                "event" => event.event = Some(value.to_owned()),
                "data" => data.push(value.to_owned()),
                _ => {}
            }
        }
        event.data = data.join("\n");
        event.comment = comments.join("\n");
        Some(event)
    }
}
