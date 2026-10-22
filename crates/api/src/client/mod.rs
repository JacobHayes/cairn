//! The Rust client of the API (ARCHITECTURE, HTTP API: the Rust client for integration
//! tests): what this crate's tests and the multiplayer testbed (6.1) talk to a server with.
//! H6's subscription tracking ([`tracker`]) knows no transport: the HTTP [`Client`] drives
//! it. A stale patch is decided by the server (H5), so the client sends each patch once.

pub mod api;
pub mod sse;
pub mod tracker;
pub mod transport;

pub use api::{Client, ClientError, Refused, Subscription};
pub use sse::{EventStream, Opened, SseEvent};
pub use tracker::Tracker;
pub use transport::{Reply, Transport, TransportError};
