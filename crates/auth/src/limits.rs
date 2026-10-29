//! The auth crate's limits (PRACTICES, Explicit limits).

use std::time::Duration;

/// An outbound login call (an OIDC discovery or code exchange, the
/// IAP key set, a Google token exchange), and a whole request to the IAP relay's
/// endpoints, its body read included: a sign-in provider this slow is down, and the
/// sign-in fails.
pub const PROVIDER_CALL_DURATION_MAX: Duration = Duration::from_secs(10);
/// Requests in flight at the IAP relay's endpoints, per process: each sign-in or refresh is
/// one short request, so tens of users' clients stay far below this; since the token
/// endpoint, registration and the metadata are reachable without IAP, a flood of them holds
/// at most this many of the process's requests in flight.
pub const RELAY_REQUEST_IN_FLIGHT_COUNT_MAX: u32 = 16;
/// How soon after a try at fetching Google's IAP key set, failed or not, another may be
/// made: rotation is rare, and neither a stream of forged key ids nor an outage at Google
/// may fetch on every request.
pub const IAP_KEY_SET_REFETCH_INTERVAL_MIN: Duration = Duration::from_secs(60);
/// How long a fetched IAP key set is trusted before it is fetched again: a key Google
/// withdraws stops verifying within this, at one fetch an hour.
pub const IAP_KEY_SET_AGE_MAX: Duration = Duration::from_hours(1);
/// The longest an IAP assertion may last, issue to expiry, before the clock skew at either
/// end: IAP's own last ten minutes, so a longer one was not issued as IAP issues them.
pub const IAP_ASSERTION_LIFETIME_MAX: Duration = Duration::from_mins(10);
/// The clock skew allowed between IAP and this machine when reading an assertion's issue
/// and expiry times: IAP's assertions last ten minutes, and NTP keeps clocks well inside this.
pub const IAP_ASSERTION_CLOCK_SKEW_MAX: Duration = Duration::from_secs(30);
