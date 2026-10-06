//! The SSE revision stream (ARCHITECTURE, HTTP API: SSE; H6): `GET /events/stream?domain=...`
//! sends a `tick` event for each revision the notifier announces, starting with the current
//! revision of everything watched, so a commit between a client's fetch and its
//! subscription is never missed; a domain deleted meanwhile starts at revision 0. Nothing else is streamed: a client refetches what moved.
//!
//! A pump task per stream drives the subscription: it takes ticks at most once per
//! coalescing interval (the notifier's), sends a heartbeat comment when the stream has been
//! quiet for `SSE_HEARTBEAT_INTERVAL`, and hands each frame to the response body through a
//! one-frame channel. A subscriber that leaves a frame untaken for `SSE_WRITE_STALL` is
//! disconnected: its subscription is dropped, freeing its place under the subscriber limit,
//! and its body ends in an error, so it reconnects and starts again from current revisions.
//! A peer that stops reading is caught here only once the socket's buffers fill; the
//! listener's `TCP_USER_TIMEOUT` (4.7) closes it at the stall, and the pump then drops it
//! as gone (DECISIONS.md, 4.2: where the SSE write stall is enforced).
//! A client that goes away is noticed at once and dropped the same way.

use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

use axum::extract::{RawQuery, State};
use axum::response::sse::Event;
use axum::response::{IntoResponse, Response, Sse};
use cairn_store::{Store, Subscription, Take};
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::Api;
use crate::error::ApiError;
use crate::limits::{SSE_HEARTBEAT_INTERVAL, SSE_WRITE_STALL};
use crate::query::{self, Params};
use crate::wire::Tick;

/// The SSE event name of a revision tick.
pub const TICK_EVENT: &str = "tick";

/// What the pump hands the response body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Frame {
    /// A revision tick.
    Tick(Tick),
    /// A comment, sent when the stream has been quiet.
    Heartbeat,
}

impl Frame {
    fn event(&self) -> Event {
        match self {
            Frame::Tick(tick) => {
                let data = serde_json::to_string(tick)
                    .unwrap_or_else(|error| unreachable!("a tick always serializes: {error}"));
                Event::default().event(TICK_EVENT).data(data)
            }
            Frame::Heartbeat => Event::default().comment("heartbeat"),
        }
    }
}

/// `GET /events/stream` (H6).
///
/// # Errors
///
/// A bad request for what it watches; 503 when the process is at its subscriber limit.
pub(crate) async fn stream<S: Store + 'static>(
    State(api): State<Api<S>>,
    RawQuery(raw): RawQuery,
) -> Result<Response, ApiError> {
    let params = Params::parse(raw.as_deref(), query::STREAM_PARAMS)?;
    let watching = query::watching(&params)?;
    let subscription = api.service.subscribe(watching).await?;
    let (frames, body) = channel();
    tokio::spawn(pump(subscription, frames));
    Ok(Sse::new(body).into_response())
}

/// The pump's end of a stream.
#[derive(Debug)]
pub struct Frames {
    sender: mpsc::Sender<Frame>,
    stalled: Arc<AtomicBool>,
}

/// The response body's end: frames as SSE events, then an error if the subscriber
/// stalled.
#[derive(Debug)]
pub struct Body {
    receiver: mpsc::Receiver<Frame>,
    stalled: Arc<AtomicBool>,
    ended: bool,
}

/// A channel holding one frame at a time, so a subscriber that stops reading stops the pump
/// within one frame.
#[must_use]
pub fn channel() -> (Frames, Body) {
    let (sender, receiver) = mpsc::channel(1);
    let stalled = Arc::new(AtomicBool::new(false));
    let frames = Frames {
        sender,
        stalled: Arc::clone(&stalled),
    };
    let body = Body {
        receiver,
        stalled,
        ended: false,
    };
    (frames, body)
}

/// The subscriber left a frame untaken for `SSE_WRITE_STALL`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stalled;

impl fmt::Display for Stalled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the subscriber took no write for {} s (sse_write_stall)",
            SSE_WRITE_STALL.as_secs()
        )
    }
}

impl std::error::Error for Stalled {}

impl Body {
    /// The next frame, as the response body reads it: none once the pump has ended.
    pub async fn next_frame(&mut self) -> Option<Result<Frame, Stalled>> {
        std::future::poll_fn(|context| self.poll_frame(context)).await
    }

    fn poll_frame(&mut self, context: &mut Context<'_>) -> Poll<Option<Result<Frame, Stalled>>> {
        if self.ended {
            return Poll::Ready(None);
        }
        if self.stalled.load(Ordering::SeqCst) {
            self.ended = true;
            return Poll::Ready(Some(Err(Stalled)));
        }
        self.receiver.poll_recv(context).map(|frame| frame.map(Ok))
    }
}

impl futures_core::Stream for Body {
    type Item = Result<Event, Stalled>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let body = self.get_mut();
        body.poll_frame(context)
            .map(|frame| frame.map(|frame| frame.map(|frame| frame.event())))
    }
}

/// Why the pump stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stopped {
    /// The subscriber went away.
    Gone,
    /// The subscriber left a frame untaken for `SSE_WRITE_STALL`.
    Stalled,
}

/// H6: hands `subscription`'s ticks to `frames` until the subscriber goes away or stalls;
/// the subscription is dropped when it returns, which unsubscribes it.
pub async fn pump(subscription: Subscription, frames: Frames) -> Stopped {
    let started = Instant::now();
    let mut quiet_since = started;
    loop {
        let batch = match subscription.take(started.elapsed()) {
            // The first take is the complete current set (a domain at 0 is gone); it
            // streams as ticks like any other.
            Take::Current(ticks) | Take::Ticks(ticks) => ticks
                .into_iter()
                .map(|tick| Frame::Tick(tick.into()))
                .collect(),
            Take::Wait { until } => {
                tokio::select! {
                    () = tokio::time::sleep_until(started + until) => continue,
                    () = frames.sender.closed() => return Stopped::Gone,
                }
            }
            Take::Empty => {
                tokio::select! {
                    () = subscription.ready() => continue,
                    () = tokio::time::sleep_until(quiet_since + SSE_HEARTBEAT_INTERVAL) => {
                        vec![Frame::Heartbeat]
                    }
                    () = frames.sender.closed() => return Stopped::Gone,
                }
            }
        };
        for frame in batch {
            if let Err(stopped) = frames.send(frame).await {
                return stopped;
            }
        }
        quiet_since = Instant::now();
    }
}

impl Frames {
    async fn send(&self, frame: Frame) -> Result<(), Stopped> {
        match self.sender.send_timeout(frame, SSE_WRITE_STALL).await {
            Ok(()) => Ok(()),
            Err(mpsc::error::SendTimeoutError::Closed(_)) => Err(Stopped::Gone),
            Err(mpsc::error::SendTimeoutError::Timeout(_)) => {
                self.stalled.store(true, Ordering::SeqCst);
                Err(Stopped::Stalled)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::time::Duration;

    use cairn_schema::{Domain, Revision, RevisionOf};
    use cairn_store::{InProcessNotifier, Notifier, Revisions, Watch};

    use super::*;

    fn journey(id: &str) -> RevisionOf {
        RevisionOf::Domain(Domain::Journey(id.parse().unwrap()))
    }

    fn revision(number: u32) -> Revision {
        Revision::try_from(number).unwrap()
    }

    /// A subscription watching every journey, seeded with `j_one` at revision 1.
    fn subscribed(notifier: &InProcessNotifier) -> Subscription {
        let subscription = notifier
            .subscribe(BTreeSet::from([Watch::Journeys]))
            .unwrap();
        let mut current = Revisions::default();
        current
            .journeys
            .insert("j_one".parse().unwrap(), revision(1));
        subscription.seed(&current);
        subscription
    }

    fn tick(of: &RevisionOf, number: u32) -> Frame {
        Frame::Tick(Tick {
            of: of.clone(),
            revision: revision(number),
        })
    }

    /// H6: current revisions first, then each commit's tick, coalesced within the interval;
    /// a quiet stream sends a heartbeat.
    #[tokio::test(start_paused = true)]
    async fn ticks_come_current_first_then_coalesced_and_a_quiet_stream_beats() {
        let notifier = InProcessNotifier::new();
        let (frames, mut body) = channel();
        tokio::spawn(pump(subscribed(&notifier), frames));
        assert_eq!(
            body.next_frame().await,
            Some(Ok(tick(&journey("j_one"), 1)))
        );
        let started = Instant::now();
        notifier.publish(&journey("j_one"), revision(2));
        notifier.publish(&journey("j_one"), revision(3));
        assert_eq!(
            body.next_frame().await,
            Some(Ok(tick(&journey("j_one"), 3)))
        );
        assert!(started.elapsed() >= cairn_store::limits::SSE_COALESCING_INTERVAL);
        let quiet = Instant::now();
        assert_eq!(body.next_frame().await, Some(Ok(Frame::Heartbeat)));
        assert_eq!(quiet.elapsed(), SSE_HEARTBEAT_INTERVAL);
    }

    /// A subscriber that takes no frame for the write stall is disconnected: its pump stops,
    /// its subscription is dropped, and its body ends in an error.
    #[tokio::test(start_paused = true)]
    async fn a_stalled_subscriber_is_disconnected_and_unsubscribed() {
        let notifier = InProcessNotifier::new();
        let (frames, mut body) = channel();
        let pumping = tokio::spawn(pump(subscribed(&notifier), frames));
        tokio::time::sleep(Duration::from_millis(1)).await;
        notifier.publish(&journey("j_one"), revision(2));
        let started = Instant::now();
        assert_eq!(pumping.await.unwrap(), Stopped::Stalled);
        let coalesced = cairn_store::limits::SSE_COALESCING_INTERVAL;
        assert_eq!(
            started.elapsed() + Duration::from_millis(1),
            SSE_WRITE_STALL + coalesced,
            "the tick waited out the interval begun at the first take, then the stall"
        );
        assert_eq!(notifier.subscriber_count(), 0);
        assert_eq!(body.next_frame().await, Some(Err(Stalled)));
        assert_eq!(body.next_frame().await, None);
    }

    /// A subscriber that goes away is dropped at once, ticks or no ticks.
    #[tokio::test(start_paused = true)]
    async fn a_subscriber_that_goes_away_is_unsubscribed() {
        let notifier = InProcessNotifier::new();
        let (frames, mut body) = channel();
        let pumping = tokio::spawn(pump(subscribed(&notifier), frames));
        assert_eq!(
            body.next_frame().await,
            Some(Ok(tick(&journey("j_one"), 1)))
        );
        drop(body);
        assert_eq!(pumping.await.unwrap(), Stopped::Gone);
        assert_eq!(notifier.subscriber_count(), 0);
    }
}
