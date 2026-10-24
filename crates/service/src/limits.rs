//! The service's limits (PRACTICES, Explicit limits): hard-coded and named, each the bound of
//! the loop that enforces it.

/// Attempts to commit one stale patch at moved revisions, per request: a runaway stop, not a
/// budget. Each attempt needs another commit to land between the service's load and its
/// commit, on the patch's own domain or one it checks
/// (decisions/2026-10-22-the-server-applies-a-stale-write-that-overlaps-nothing.md).
pub const STALE_ATTEMPT_COUNT_MAX: u32 = 8;
