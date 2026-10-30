//! The listener (ARCHITECTURE, Build, run, deploy: one port for API, MCP, assistant, SSE,
//! and UI). Every accepted connection gets `TCP_USER_TIMEOUT` at the SSE write stall and
//! TCP keepalive (decisions/2026-10-06-where-the-sse-write-stall-is-enforced.md): data the
//! peer leaves unacknowledged for the stall, or a closed receive window that long, closes
//! the connection in the kernel, hyper drops the response body, and the SSE pump, which
//! watches its channel close, drops the subscription at once.

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::Router;
use axum::body::{Body, BodyDataStream};
use axum::http::{Request, header::CONTENT_TYPE};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::serve::ListenerExt;
use futures_core::Stream;
use socket2::{SockRef, TcpKeepalive};
use tokio::net::{TcpListener, TcpStream};

use crate::limits::{
    KEEPALIVE_IDLE, KEEPALIVE_INTERVAL, SHUTDOWN_DRAIN_MAX, UNACKNOWLEDGED_DURATION_MAX,
};

/// Sets an accepted connection's socket options: keepalive, `TCP_USER_TIMEOUT` at
/// `unacknowledged` (the binary's is [`UNACKNOWLEDGED_DURATION_MAX`]), and no Nagle delay (an
/// SSE tick is one small write that should leave at once).
///
/// # Errors
///
/// When the kernel refuses an option.
pub fn configure(stream: &TcpStream, unacknowledged: Duration) -> io::Result<()> {
    let socket = SockRef::from(stream);
    let keepalive = TcpKeepalive::new()
        .with_time(KEEPALIVE_IDLE)
        .with_interval(KEEPALIVE_INTERVAL);
    socket.set_tcp_keepalive(&keepalive)?;
    set_user_timeout(&socket, unacknowledged)?;
    socket.set_tcp_nodelay(true)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_user_timeout(socket: &SockRef<'_>, unacknowledged: Duration) -> io::Result<()> {
    socket.set_tcp_user_timeout(Some(unacknowledged))
}

/// Other kernels have no `TCP_USER_TIMEOUT`: there, a peer that stops reading is caught
/// once its buffers fill (the API's own stall), and a proxy's idle timeout must stand in
/// for the rest (decisions/2026-10-06-where-the-sse-write-stall-is-enforced.md, what
/// would change it). [`warn_unsupported`] says so at startup.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
#[allow(clippy::unnecessary_wraps)] // Linux's can fail, and `configure` calls either.
fn set_user_timeout(_socket: &SockRef<'_>, _unacknowledged: Duration) -> io::Result<()> {
    Ok(())
}

/// Logs, once at startup and after `listening`, that this platform cannot close a stalled
/// connection at the socket.
pub fn warn_unsupported() {
    if cfg!(not(any(target_os = "linux", target_os = "android"))) {
        tracing::warn!(
            "this platform has no TCP_USER_TIMEOUT: a peer that stops reading holds its SSE \
             slot until its buffers fill"
        );
    }
}

/// An event stream's body, ended cleanly when the server stops: the client sees the stream
/// close between two events and reopens it against whatever comes next.
struct EndsAtShutdown {
    body: BodyDataStream,
    stopping: Pin<Box<dyn Future<Output = ()> + Send>>,
}

impl Stream for EndsAtShutdown {
    type Item = <BodyDataStream as Stream>::Item;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.stopping.as_mut().poll(context).is_ready() {
            return Poll::Ready(None);
        }
        Pin::new(&mut self.body).poll_next(context)
    }
}

/// Wraps each event stream's body (any `text/event-stream` response: the revision stream and
/// the MCP endpoint's) so it ends when the server begins to stop. Ordinary responses finish
/// by themselves; these never would, and graceful shutdown would wait on them.
async fn end_streams_at_shutdown(
    mut stopped: tokio::sync::watch::Receiver<bool>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let response = next.run(request).await;
    let streaming = response
        .headers()
        .get(CONTENT_TYPE)
        .is_some_and(|kind| kind.as_bytes().starts_with(b"text/event-stream"));
    if !streaming {
        return response;
    }
    let (head, body) = response.into_parts();
    let ending = EndsAtShutdown {
        body: body.into_data_stream(),
        stopping: Box::pin(async move {
            let _ = stopped.wait_for(|stopped| *stopped).await;
        }),
    };
    Response::from_parts(head, Body::from_stream(ending))
}

/// Serves `app` on `listener` until `shutdown` resolves, every connection configured and its
/// peer recorded for the auth layer. Requests in flight then get up to the request duration
/// to finish; event streams end at once, so they do not hold the stop up, and connections
/// still open after the duration are dropped.
///
/// # Errors
///
/// When serving fails.
pub async fn serve(
    listener: TcpListener,
    app: Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    serve_with_user_timeout(listener, app, UNACKNOWLEDGED_DURATION_MAX, shutdown).await
}

/// [`serve`] with `TCP_USER_TIMEOUT` at `unacknowledged` instead of the limit, so a test
/// watches the kernel close a stalled connection without waiting out the write stall
/// (decisions/2026-10-06-where-the-sse-write-stall-is-enforced.md). The
/// binary serves through [`serve`] only.
///
/// # Errors
///
/// When serving fails.
pub async fn serve_with_user_timeout(
    listener: TcpListener,
    app: Router,
    unacknowledged: Duration,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    let listener = listener.tap_io(move |stream| {
        if let Err(error) = configure(stream, unacknowledged) {
            tracing::warn!(%error, "an accepted connection's socket options were not set");
        }
    });
    let (stopping, stopped) = tokio::sync::watch::channel(false);
    let streams = stopped.clone();
    let app = app.layer(middleware::from_fn(move |request, next| {
        end_streams_at_shutdown(streams.clone(), request, next)
    }));
    let serving = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        let mut stopped = stopped;
        let _ = stopped.wait_for(|stopped| *stopped).await;
    })
    .into_future();
    tokio::pin!(serving);
    tokio::select! {
        outcome = &mut serving => return outcome,
        () = shutdown => {}
    }
    tracing::info!("stopping: waiting for requests in flight");
    let _ = stopping.send(true);
    if let Ok(outcome) = tokio::time::timeout(SHUTDOWN_DRAIN_MAX, serving).await {
        return outcome;
    }
    tracing::info!("stopped with connections still open");
    Ok(())
}

#[cfg(all(test, any(target_os = "linux", target_os = "android")))]
mod tests {
    use super::*;

    /// An accepted connection carries the write stall as its user timeout, and
    /// keepalive at the write stall, probing at the heartbeat interval
    /// (decisions/2026-10-06-where-the-sse-write-stall-is-enforced.md).
    #[tokio::test]
    async fn an_accepted_connection_carries_the_stall_and_keepalive() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let _client = TcpStream::connect(address).await.unwrap();
        let (accepted, _) = listener.accept().await.unwrap();
        configure(&accepted, UNACKNOWLEDGED_DURATION_MAX).unwrap();
        let socket = SockRef::from(&accepted);
        assert_eq!(
            socket.tcp_user_timeout().unwrap(),
            Some(UNACKNOWLEDGED_DURATION_MAX)
        );
        assert!(socket.keepalive().unwrap());
        assert_eq!(socket.tcp_keepalive_time().unwrap(), KEEPALIVE_IDLE);
        assert_eq!(socket.tcp_keepalive_interval().unwrap(), KEEPALIVE_INTERVAL);
        assert!(socket.tcp_nodelay().unwrap());
    }
}
