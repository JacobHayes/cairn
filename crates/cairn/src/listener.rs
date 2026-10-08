//! The listener (ARCHITECTURE, Build, run, deploy: one port for API, MCP, assistant, SSE,
//! and UI). Every accepted connection gets `TCP_USER_TIMEOUT` at the SSE write stall and
//! TCP keepalive (decisions/2026-10-06-where-the-sse-write-stall-is-enforced.md): data the
//! peer leaves unacknowledged for the stall, or a closed receive window that long, closes
//! the connection in the kernel, hyper drops the response body, and the SSE pump, which
//! watches its channel close, drops the subscription at once.

use std::future::Future;
use std::io;
use std::net::SocketAddr;

use axum::Router;
use axum::serve::ListenerExt;
use socket2::{SockRef, TcpKeepalive};
use tokio::net::{TcpListener, TcpStream};

use crate::limits::{
    KEEPALIVE_IDLE, KEEPALIVE_INTERVAL, SHUTDOWN_DRAIN_MAX, UNACKNOWLEDGED_DURATION_MAX,
};

/// Sets an accepted connection's socket options: keepalive, `TCP_USER_TIMEOUT`, and no
/// Nagle delay (an SSE tick is one small write that should leave at once).
///
/// # Errors
///
/// When the kernel refuses an option.
pub fn configure(stream: &TcpStream) -> io::Result<()> {
    let socket = SockRef::from(stream);
    let keepalive = TcpKeepalive::new()
        .with_time(KEEPALIVE_IDLE)
        .with_interval(KEEPALIVE_INTERVAL);
    socket.set_tcp_keepalive(&keepalive)?;
    set_user_timeout(&socket)?;
    socket.set_tcp_nodelay(true)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_user_timeout(socket: &SockRef<'_>) -> io::Result<()> {
    socket.set_tcp_user_timeout(Some(UNACKNOWLEDGED_DURATION_MAX))
}

/// Other kernels have no `TCP_USER_TIMEOUT`: there, a peer that stops reading is caught
/// once its buffers fill (the API's own stall), and a proxy's idle timeout must stand in
/// for the rest (decisions/2026-10-06-where-the-sse-write-stall-is-enforced.md, what
/// would change it). [`warn_unsupported`] says so at startup.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn set_user_timeout(_socket: &SockRef<'_>) -> io::Result<()> {
    Ok(())
}

/// Logs, once at startup, that this platform cannot close a stalled connection at the
/// socket.
pub fn warn_unsupported() {
    if cfg!(not(any(target_os = "linux", target_os = "android"))) {
        tracing::warn!(
            "this platform has no TCP_USER_TIMEOUT: a peer that stops reading holds its SSE \
             slot until its buffers fill"
        );
    }
}

/// Serves `app` on `listener` until `shutdown` resolves, every connection configured and its
/// peer recorded for the auth layer. Requests in flight then get up to the request duration
/// to finish; connections still open after it (SSE streams) are dropped.
///
/// # Errors
///
/// When serving fails.
pub async fn serve(
    listener: TcpListener,
    app: Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    let listener = listener.tap_io(|stream| {
        if let Err(error) = configure(stream) {
            tracing::warn!(%error, "an accepted connection's socket options were not set");
        }
    });
    let (stopping, stopped) = tokio::sync::watch::channel(false);
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
    tracing::info!("stopped with connections still open (event streams)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An accepted connection carries the write stall as its user timeout, and
    /// keepalive at the write stall, probing at the heartbeat interval
    /// (decisions/2026-10-06-where-the-sse-write-stall-is-enforced.md).
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[tokio::test]
    async fn an_accepted_connection_carries_the_stall_and_keepalive() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let _client = TcpStream::connect(address).await.unwrap();
        let (accepted, _) = listener.accept().await.unwrap();
        configure(&accepted).unwrap();
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
