//! What every write carries besides its input: who is acting and when.

use cairn_schema::{Actor, Timestamp};

/// One request's context (H2; ARCHITECTURE, Assumptions: today per request). The host fills
/// it in once per request, from its auth layer and its clock; the service never reads a
/// clock, so a request sees one today however long it runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    /// Who is acting: a user, or an agent acting for one (H2).
    pub actor: Actor,
    /// When the request arrived: the commit time on its events, and, in the deployment's
    /// time zone, its today.
    pub now: Timestamp,
}
