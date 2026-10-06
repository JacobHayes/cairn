//! Who a sign-in is (ARCHITECTURE, Auth): the identity an auth provider vouches for, which
//! the auth crate resolves to a user and the API, MCP, and service layers share (H1, H3).

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::id::Slug;
use crate::text::{Email, Title};

/// An account at one auth provider, keyed by the provider's configured name and its own
/// subject for the account. A user owns many (ARCHITECTURE, Auth).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    /// The provider, by its configured name.
    pub provider: Slug,
    /// The provider's stable subject for the account.
    pub subject: Title,
    /// The name the provider gives the account.
    pub display: Title,
    /// H3: only the emails the provider's issuer marks verified; an unverified email never
    /// matches an entity or links a user.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub verified_emails: BTreeSet<Email>,
}
