//! The auth crate's limits (PRACTICES, Explicit limits).

use std::time::Duration;

/// An outbound identity call (an OIDC discovery or code exchange, a Tailscale whois, the
/// IAP key set): an
/// identity provider this slow is down, and the sign-in fails.
pub const IDENTITY_CALL_DURATION_MAX: Duration = Duration::from_secs(10);
/// How soon an IAP assertion naming a key Cairn has not seen may fetch Google's key set
/// again: rotation is rare, and a stream of forged key ids must not fetch on every request.
pub const IAP_KEY_SET_REFETCH_INTERVAL_MIN: Duration = Duration::from_secs(60);
/// The clock skew allowed between IAP and this machine when reading an assertion's issue
/// and expiry times: IAP's assertions last ten minutes, and NTP keeps clocks well inside this.
pub const IAP_ASSERTION_CLOCK_SKEW_MAX: Duration = Duration::from_secs(30);
