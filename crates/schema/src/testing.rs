//! Property-test strategies for every schema type (PRACTICES, Property tests), generating
//! within the limits and with generic content only. Behind the `testing` feature so the
//! crates that build on these types can generate them too. A strategy that produces a value
//! its type rejects is a bug in the strategy, so the helpers panic rather than filter.
#![allow(clippy::missing_panics_doc)]

use proptest::prelude::*;

use crate::{
    AgentId, AttachmentKey, Date, Days, Email, EntityKey, InsertionKey, JourneyId, KindKey,
    Markdown, NodeKey, PatchId, Path, ProposalId, Reason, Revision, RoleKey, RouteId, Slug,
    Timestamp, Title, Url, UserId, VersionNumber, Weight,
};

const SLUG_REGEX: &str = "[a-z0-9][a-z0-9_-]{0,11}";

/// Parses text a strategy produced in the type's own grammar.
pub(crate) fn parsed<T: std::str::FromStr>(text: &str) -> T
where
    T::Err: std::fmt::Debug,
{
    match text.parse() {
        Ok(value) => value,
        Err(error) => panic!("strategy produced {text:?}, which does not parse: {error:?}"),
    }
}

/// An id slug.
pub fn arb_slug() -> impl Strategy<Value = Slug> {
    SLUG_REGEX.prop_map(|text| parsed(&text))
}

/// A path of one to four segments.
pub fn arb_path() -> impl Strategy<Value = Path> {
    prop::collection::vec(SLUG_REGEX, 1..=4).prop_map(|segments| parsed(&segments.join("/")))
}

macro_rules! arb_prefixed {
    ($(#[$doc:meta])* $function:ident, $type:ty, $prefix:literal) => {
        $(#[$doc])*
        pub fn $function() -> impl Strategy<Value = $type> {
            "[a-z0-9]{1,8}".prop_map(|body| parsed(&format!("{}{body}", $prefix)))
        }
    };
}

arb_prefixed!(
    /// A node key.
    arb_node_key, NodeKey, "n_"
);
arb_prefixed!(
    /// A role key.
    arb_role_key, RoleKey, "r_"
);
arb_prefixed!(
    /// A participation kind key.
    arb_kind_key, KindKey, "k_"
);
arb_prefixed!(
    /// An entity key.
    arb_entity_key, EntityKey, "e_"
);
arb_prefixed!(
    /// An attachment key.
    arb_attachment_key, AttachmentKey, "a_"
);
arb_prefixed!(
    /// An insertion key.
    arb_insertion_key, InsertionKey, "i_"
);
arb_prefixed!(
    /// A journey id.
    arb_journey_id, JourneyId, "j_"
);
arb_prefixed!(
    /// A patch id.
    arb_patch_id, PatchId, "p_"
);
arb_prefixed!(
    /// A proposal id.
    arb_proposal_id, ProposalId, "pr_"
);
arb_prefixed!(
    /// A user id.
    arb_user_id, UserId, "u_"
);
arb_prefixed!(
    /// An agent id.
    arb_agent_id, AgentId, "ag_"
);

/// A route id.
pub fn arb_route_id() -> impl Strategy<Value = RouteId> {
    SLUG_REGEX.prop_map(|text| parsed(&text))
}

/// A title of a few generic words.
pub fn arb_title() -> impl Strategy<Value = Title> {
    "[A-Z][a-z]{1,8}( [a-z]{1,8}){0,3}".prop_map(|text| parsed(&text))
}

/// A short markdown body, possibly several lines.
pub fn arb_markdown() -> impl Strategy<Value = Markdown> {
    "[A-Za-z]{1,8}( [a-z*_]{1,8}){0,6}(\n\n[a-z]{1,8}){0,2}".prop_map(|text| parsed(&text))
}

/// A reason.
pub fn arb_reason() -> impl Strategy<Value = Reason> {
    "[A-Z][a-z]{1,8}( [a-z]{1,8}){0,4}".prop_map(|text| parsed(&text))
}

/// A URL.
pub fn arb_url() -> impl Strategy<Value = Url> {
    "https://example\\.(org|com)/[a-z0-9/-]{0,16}".prop_map(|text| parsed(&text))
}

/// A normalized email address.
pub fn arb_email() -> impl Strategy<Value = Email> {
    "[a-z]{1,8}@example\\.(org|com)".prop_map(|text| parsed(&text))
}

/// A revision.
pub fn arb_revision() -> impl Strategy<Value = Revision> {
    (0u32..10_000).prop_map(|value| Revision::try_from(value).unwrap_or(Revision::NONE))
}

/// A route version number.
pub fn arb_version_number() -> impl Strategy<Value = VersionNumber> {
    (1u32..100).prop_map(|value| VersionNumber::try_from(value).unwrap_or(VersionNumber::FIRST))
}

/// A day count within `offset_days_max`.
pub fn arb_days() -> impl Strategy<Value = Days> {
    (0..=crate::limits::OFFSET_DAYS_MAX)
        .prop_filter_map("within the limit", |value| Days::try_from(value).ok())
}

/// A weight within `weight_max`.
pub fn arb_weight() -> impl Strategy<Value = Weight> {
    (0..=crate::limits::WEIGHT_MAX)
        .prop_filter_map("within the limit", |value| Weight::try_from(value).ok())
}

/// A calendar date between 2020 and 2039.
pub fn arb_date() -> impl Strategy<Value = Date> {
    (2020i16..2040, 1i8..=12, 1i8..=28)
        .prop_map(|(year, month, day)| Date::constant(year, month, day))
}

/// A timestamp, whole seconds, between 2020 and 2039.
pub fn arb_timestamp() -> impl Strategy<Value = Timestamp> {
    (1_577_836_800i64..2_208_988_800)
        .prop_map(|seconds| Timestamp::from_second(seconds).unwrap_or(Timestamp::UNIX_EPOCH))
}

mod derived;
mod documents;
mod model;
mod node;
mod writes;

pub use derived::*;
pub use documents::*;
pub use model::*;
pub use node::*;
pub use writes::*;
