//! How long what the auth crate issues lasts (3.2, Decisions left to the implementer:
//! session and token lifetimes). Agent tokens last until revoked: a token is an identity
//! (ARCHITECTURE, Auth), and a script holding one should not stop on a timer.

use std::time::Duration;

/// A browser session: two weeks from sign-in, then sign in again. Not extended by use, so
/// a stolen cookie has a fixed end.
pub const SESSION_LIFETIME: Duration = Duration::from_hours(14 * 24);
/// A login or consent in progress: the time a person takes at a provider's login page.
pub const BROWSER_STEP_LIFETIME: Duration = Duration::from_mins(10);
/// An authorization code of the built-in OAuth server, exchanged at once by its client.
pub const AUTHORIZATION_CODE_LIFETIME: Duration = Duration::from_secs(60);
