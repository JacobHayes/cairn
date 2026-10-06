//! The typed client: reads, domain patches with H5's safe retry, and subscriptions whose
//! ticks feed H6's tracking, over the HTTP transport and the same endpoint table the router
//! serves.

use std::fmt;

use axum::http::{Method, StatusCode};
use cairn_schema::{Deployment, Journey, JourneyId, Markdown, Patch, PatchTarget, Rejection};
use serde::de::DeserializeOwned;

use super::retry::{self, Landed, Refused};
use super::sse::{EventStream, Opened};
use super::transport::{Reply, Transport, TransportError};
use crate::endpoints::{self, Endpoint};
use crate::query::WatchName;
use crate::stream::TICK_EVENT;
use crate::wire::{PatchAnswer, PatchRequest, Problem, Tick};

/// A request that did not get the answer it asked for (a rejected patch is a
/// [`Refused::Rejected`], not this).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientError {
    /// The server answered a problem.
    Problem {
        /// The status.
        status: StatusCode,
        /// The problem.
        problem: Problem,
    },
    /// The server answered something the API does not describe.
    Unexpected {
        /// The status.
        status: StatusCode,
        /// The body, as text.
        body: String,
    },
    /// The request could not be sent or its answer read.
    Transport(TransportError),
    /// The patch targets a proposal, which is not a domain patch.
    NotADomainPatch,
}

impl fmt::Display for ClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::Problem { status, problem } => {
                write!(formatter, "{status}: {}", problem.message)
            }
            ClientError::Unexpected { status, body } => write!(formatter, "{status}: {body}"),
            ClientError::Transport(error) => error.fmt(formatter),
            ClientError::NotADomainPatch => formatter.write_str("a proposal is not a domain"),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<TransportError> for ClientError {
    fn from(error: TransportError) -> Self {
        ClientError::Transport(error)
    }
}

/// A reply that is not the success it should be, as an error.
fn unexpected(reply: &Reply) -> ClientError {
    match reply.json::<Problem>() {
        Ok(problem) => ClientError::Problem {
            status: reply.status,
            problem,
        },
        Err(_) => ClientError::Unexpected {
            status: reply.status,
            body: String::from_utf8_lossy(&reply.body).into_owned(),
        },
    }
}

/// A client of one server, as one caller.
#[derive(Clone, Debug)]
pub struct Client {
    transport: Transport,
}

impl Client {
    /// Over `transport`.
    #[must_use]
    pub fn new(transport: Transport) -> Self {
        Self { transport }
    }

    /// The transport, for raw requests.
    #[must_use]
    pub fn transport(&self) -> &Transport {
        &self.transport
    }

    /// GETs `endpoint` with its placeholders filled by `segments` and `query` (empty, or
    /// `name=value&...`) appended.
    ///
    /// # Errors
    ///
    /// A problem, an unexpected answer, or a transport failure.
    pub async fn read<T: DeserializeOwned>(
        &self,
        endpoint: &Endpoint,
        segments: &[&str],
        query: &str,
    ) -> Result<T, ClientError> {
        let mut target = endpoint.path_with(segments);
        if !query.is_empty() {
            target.push('?');
            target.push_str(query);
        }
        let reply = self.transport.send(Method::GET, &target, None).await?;
        if reply.status != StatusCode::OK {
            return Err(unexpected(&reply));
        }
        Ok(reply.json()?)
    }

    /// A journey.
    ///
    /// # Errors
    ///
    /// As [`Client::read`].
    pub async fn journey(&self, id: &JourneyId) -> Result<Journey, ClientError> {
        self.read(&endpoints::JOURNEY, &[id.as_str()], "").await
    }

    /// The deployment.
    ///
    /// # Errors
    ///
    /// As [`Client::read`].
    pub async fn deployment(&self) -> Result<Deployment, ClientError> {
        self.read(&endpoints::DEPLOYMENT, &[], "").await
    }

    /// Submits a domain patch once.
    ///
    /// # Errors
    ///
    /// Its rejection, or why it could not be submitted.
    pub async fn submit(
        &self,
        patch: Patch,
        note: Option<Markdown>,
    ) -> Result<PatchAnswer, Refused<ClientError>> {
        let target = match &patch.target {
            PatchTarget::Journey(id) => endpoints::PATCH_JOURNEY.path_with(&[id.as_str()]),
            PatchTarget::Route(id) => endpoints::PATCH_ROUTE.path_with(&[id.as_str()]),
            PatchTarget::Deployment => endpoints::PATCH_DEPLOYMENT.path_with(&[]),
            PatchTarget::Proposal { .. } => {
                return Err(Refused::Failed(ClientError::NotADomainPatch));
            }
        };
        let body = serde_json::to_value(PatchRequest { patch, note })
            .map_err(|error| Refused::Failed(TransportError(error.to_string()).into()))?;
        let reply = self
            .transport
            .send(Method::POST, &target, Some(&body))
            .await;
        let reply = reply.map_err(|error| Refused::Failed(error.into()))?;
        match reply.status {
            StatusCode::OK => reply.json().map_err(|error| Refused::Failed(error.into())),
            StatusCode::CONFLICT | StatusCode::UNPROCESSABLE_ENTITY => {
                match reply.json::<Rejection>() {
                    Ok(rejection) => Err(Refused::Rejected(rejection)),
                    Err(_) => Err(Refused::Failed(unexpected(&reply))),
                }
            }
            _ => Err(Refused::Failed(unexpected(&reply))),
        }
    }

    /// H5: submits a domain patch, resubmitting it on its own while it is stale and safe
    /// to retry.
    ///
    /// # Errors
    ///
    /// Its rejection (a stale one when retrying was not safe), or why it could not be
    /// submitted.
    pub async fn patch(
        &self,
        patch: Patch,
        note: Option<Markdown>,
    ) -> Result<Landed, Refused<ClientError>> {
        retry::submit(patch, |patch| self.submit(patch, note.clone())).await
    }

    /// H6: subscribes to the revision ticks of what `watching` names.
    ///
    /// # Errors
    ///
    /// A refusal (the subscriber limit), or a transport failure.
    pub async fn subscribe(&self, watching: &[WatchName]) -> Result<Subscription, ClientError> {
        let query: Vec<String> = watching
            .iter()
            .map(|name| format!("domain={name}"))
            .collect();
        let target = format!("{}?{}", endpoints::STREAM.path, query.join("&"));
        match self.transport.stream(&target).await? {
            Opened::Stream(stream) => Ok(Subscription { stream }),
            Opened::Refused(reply) => Err(unexpected(&reply)),
        }
    }
}

/// An open subscription: its ticks, current revisions first.
#[derive(Debug)]
pub struct Subscription {
    stream: Box<EventStream>,
}

impl Subscription {
    /// The next tick, past any heartbeat; none when the server ended the stream.
    ///
    /// # Errors
    ///
    /// A transport failure, or a tick that does not parse.
    pub async fn next_tick(&mut self) -> Result<Option<Tick>, ClientError> {
        while let Some(event) = self.stream.next_event().await? {
            if event.event.as_deref() == Some(TICK_EVENT) {
                let tick = serde_json::from_str(&event.data).map_err(|error| {
                    ClientError::Transport(TransportError(format!("a tick: {error}")))
                })?;
                return Ok(Some(tick));
            }
        }
        Ok(None)
    }
}
