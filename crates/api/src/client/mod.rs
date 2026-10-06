//! The Rust client of the API (ARCHITECTURE, HTTP API: the Rust client for integration
//! tests): what this crate's tests and the multiplayer testbed (6.1) talk to a server with.
//! H5's safe retry ([`retry`]) and H6's subscription tracking ([`tracker`]) know no
//! transport: the HTTP [`Client`] drives them, and [`in_process`] drives the retry against
//! the service directly.

pub mod api;
pub mod in_process;
pub mod retry;
pub mod sse;
pub mod tracker;
pub mod transport;

pub use api::{Client, ClientError, Subscription};
pub use retry::{Landed, Refused};
pub use sse::{EventStream, Opened, SseEvent};
pub use tracker::Tracker;
pub use transport::{Exchange, Observer, Reply, Transport, TransportError};
