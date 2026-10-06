//! The auth crate's limits (PRACTICES, Explicit limits).

use std::time::Duration;

/// An outbound identity call (an OIDC discovery or code exchange, a Tailscale whois): an
/// identity provider this slow is down, and the sign-in fails.
pub const IDENTITY_CALL_DURATION_MAX: Duration = Duration::from_secs(10);
