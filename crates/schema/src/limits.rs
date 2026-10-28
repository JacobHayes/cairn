//! Value-level limits (PRACTICES, Explicit limits): hard-coded, enforced when a value is
//! deserialized or constructed, and named in the error when one is exceeded. Limits that a
//! patch can reach one write at a time (explicit edges in plus out, containment depth over
//! the tree, resources, notes, and links per node, a merged identity's emails and aliases,
//! identities per deployment) are also checked by validation in the engine, against the same
//! constants.

use std::fmt;
use std::time::Duration;

/// Nodes per graph: 10x headroom over a large journey.
pub const NODE_COUNT_MAX: u32 = 2_000;
/// Containment depth, which is also the most segments a path has.
pub const CONTAINMENT_DEPTH_MAX: u32 = 16;
/// Explicit edges per node, in plus out. A node's own `requires` list is held to it at
/// deserialization; the in-plus-out total is a graph-level check.
pub const EDGE_COUNT_PER_NODE_MAX: u32 = 64;
/// Mutations per patch: an import of a full graph is roughly four mutations per node.
pub const MUTATION_COUNT_PER_PATCH_MAX: u32 = 8_000;
/// Roles per graph.
pub const ROLE_COUNT_MAX: u32 = 32;
/// Participation kinds a graph declares; the built-in `owner` is not one of them.
pub const KIND_COUNT_MAX: u32 = 32;
/// Choices per decision.
pub const CHOICE_COUNT_PER_DECISION_MAX: u32 = 32;
/// Checklist items per node: a list someone ticks through in one sitting.
pub const CHECKLIST_ITEM_COUNT_PER_NODE_MAX: u32 = 50;
/// Identities per role fill or identity-list answer.
pub const IDENTITY_COUNT_PER_FILL_MAX: u32 = 100;
/// Condition tree depth: a lone clause is depth 1.
pub const CONDITION_DEPTH_MAX: u32 = 8;
/// Clauses (leaf predicates) per condition tree.
pub const CONDITION_CLAUSE_COUNT_MAX: u32 = 16;
/// Bytes in an id slug, and in a key (keys share the id limit: both are short references).
pub const ID_BYTES_MAX: u32 = 64;
/// Bytes in a title, and in the other single-line labels (names, choice labels).
pub const TITLE_BYTES_MAX: u32 = 256;
/// Bytes in free text: a description, prompt, help, note, tip, message draft, or text answer.
pub const BODY_BYTES_MAX: u32 = 64 * 1024;
/// Bytes in a link (a URL). Practical URLs stay under about 2 KiB; twice that admits long
/// signed links and stays under the 8 KiB request line common servers accept.
pub const LINK_BYTES_MAX: u32 = 4 * 1024;
/// Bytes in an email address: RFC 5321 caps a path at 256 octets, angle brackets included.
pub const EMAIL_BYTES_MAX: u32 = 254;
/// Bytes in the reason a skip, override, or bypass requires: a sentence or a short
/// paragraph; a longer account is a note.
pub const REASON_BYTES_MAX: u32 = 4 * 1024;
/// Emails per identity: work, personal, and a few former addresses, with room for a merge to
/// join two people's lists.
pub const EMAIL_COUNT_PER_IDENTITY_MAX: u32 = 16;
/// Merged keys (aliases) resolving to one identity: each merge of a duplicate adds one.
pub const ALIAS_COUNT_PER_IDENTITY_MAX: u32 = 32;
/// Identities per deployment: hundreds of journeys naming tens of people each, mostly shared.
pub const IDENTITY_COUNT_PER_DEPLOYMENT_MAX: u32 = 5_000;
/// Journeys one identity merge names: every journey referencing either identity.
pub const JOURNEY_COUNT_PER_MERGE_MAX: u32 = 1_000;
/// Resources per node: a tip, a template, an example or two.
pub const RESOURCE_COUNT_PER_NODE_MAX: u32 = 16;
/// Notes on one node, or on the journey itself: a running log of a long piece of work.
pub const NOTE_COUNT_PER_NODE_MAX: u32 = 100;
/// Links (artifacts, references, conversations) on one node, or on the journey itself.
pub const LINK_COUNT_PER_NODE_MAX: u32 = 32;
/// Days in a date offset or an estimate.
pub const OFFSET_DAYS_MAX: u32 = 365;
/// A node's weight.
pub const WEIGHT_MAX: u32 = 1_000;
/// Bytes in one graph serialized with its state. Validation checks the graph a patch
/// produces against it; a parse cannot, since an envelope around a graph at its cap is larger.
pub const GRAPH_BYTES_MAX: u32 = 16 * 1024 * 1024;
/// Bytes in a request body: the graph cap plus envelope, so a domain at its cap still fits
/// one request. Every document parse checks it first.
pub const REQUEST_BYTES_MAX: u32 = 24 * 1024 * 1024;
/// How long an API request may take before it is answered as timed out (not the SSE
/// stream, whose response starts at once): typical requests take tens of milliseconds, the
/// heaviest about a second, so past five something is stuck. It lives here beside the
/// request body cap because a turn of the assistant keeps this much of its own limit to
/// save its conversation.
pub const REQUEST_DURATION_MAX: Duration = Duration::from_secs(5);
/// Contradictory chains listed in one rejection; the rejection says when there were more.
pub const CHAIN_COUNT_PER_REJECTION_MAX: u32 = 16;
/// Explanation entries per derived value in a server response; the rest is a total.
pub const EXPLANATION_ENTRY_COUNT_MAX: u32 = 50;
/// Items in one page of a projection an agent pages (the snapshot's acting frontier, the node
/// list, history): the snapshot top-N and history page limit, the same value the
/// store pages its lists by. Callers page; the remainder is counts and keys.
pub const PAGE_ITEM_COUNT_MAX: u32 = 200;

/// Entries in each of the snapshot's lists of nodes needing attention (open decisions,
/// placeholders to break down, unassigned work, shortfalls); the counts say how many in all and
/// `list_nodes` with the matching filter lists the rest (`decisions_needed` for the decisions that
/// can be answered now). Keeps the overview small at any journey size.
pub const SNAPSHOT_LIST_ITEM_COUNT_MAX: u32 = 10;

// An import is about four mutations per node (PRACTICES, Explicit limits).
const _: () = assert!(MUTATION_COUNT_PER_PATCH_MAX == 4 * NODE_COUNT_MAX);
// A path's segments and separators fit in a few kilobytes.
// A request can carry a whole graph at its cap.
const _: () = assert!(REQUEST_BYTES_MAX > GRAPH_BYTES_MAX);
const _: () = assert!(CONTAINMENT_DEPTH_MAX * (ID_BYTES_MAX + 1) < BODY_BYTES_MAX);
// The named text limits sit between a label and free text.
const _: () = assert!(EMAIL_BYTES_MAX <= TITLE_BYTES_MAX);
const _: () = assert!(TITLE_BYTES_MAX < LINK_BYTES_MAX && LINK_BYTES_MAX < BODY_BYTES_MAX);
const _: () = assert!(TITLE_BYTES_MAX < REASON_BYTES_MAX && REASON_BYTES_MAX < BODY_BYTES_MAX);

/// A value-level limit, named in the error when it is exceeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Limit {
    /// [`NODE_COUNT_MAX`].
    NodeCount,
    /// [`CONTAINMENT_DEPTH_MAX`].
    ContainmentDepth,
    /// [`EDGE_COUNT_PER_NODE_MAX`].
    EdgeCountPerNode,
    /// [`MUTATION_COUNT_PER_PATCH_MAX`].
    MutationCountPerPatch,
    /// [`ROLE_COUNT_MAX`].
    RoleCount,
    /// [`KIND_COUNT_MAX`].
    KindCount,
    /// [`CHOICE_COUNT_PER_DECISION_MAX`].
    ChoiceCountPerDecision,
    /// [`CHECKLIST_ITEM_COUNT_PER_NODE_MAX`].
    ChecklistItemCountPerNode,
    /// [`IDENTITY_COUNT_PER_FILL_MAX`].
    IdentityCountPerFill,
    /// [`CONDITION_DEPTH_MAX`].
    ConditionDepth,
    /// [`CONDITION_CLAUSE_COUNT_MAX`].
    ConditionClauseCount,
    /// [`ID_BYTES_MAX`].
    IdBytes,
    /// [`TITLE_BYTES_MAX`].
    TitleBytes,
    /// [`BODY_BYTES_MAX`].
    BodyBytes,
    /// [`LINK_BYTES_MAX`].
    LinkBytes,
    /// [`EMAIL_BYTES_MAX`].
    EmailBytes,
    /// [`REASON_BYTES_MAX`].
    ReasonBytes,
    /// [`EMAIL_COUNT_PER_IDENTITY_MAX`].
    EmailCountPerIdentity,
    /// [`ALIAS_COUNT_PER_IDENTITY_MAX`].
    AliasCountPerIdentity,
    /// [`IDENTITY_COUNT_PER_DEPLOYMENT_MAX`].
    IdentityCountPerDeployment,
    /// [`JOURNEY_COUNT_PER_MERGE_MAX`].
    JourneyCountPerMerge,
    /// [`RESOURCE_COUNT_PER_NODE_MAX`].
    ResourceCountPerNode,
    /// [`NOTE_COUNT_PER_NODE_MAX`].
    NoteCountPerNode,
    /// [`LINK_COUNT_PER_NODE_MAX`].
    LinkCountPerNode,
    /// [`OFFSET_DAYS_MAX`].
    OffsetDays,
    /// [`WEIGHT_MAX`].
    Weight,
    /// [`GRAPH_BYTES_MAX`].
    GraphBytes,
    /// [`REQUEST_BYTES_MAX`].
    RequestBytes,
    /// [`CHAIN_COUNT_PER_REJECTION_MAX`].
    ChainCountPerRejection,
    /// [`EXPLANATION_ENTRY_COUNT_MAX`].
    ExplanationEntryCount,
}

impl Limit {
    /// Every limit, for tests that walk them all.
    pub const ALL: [Limit; 30] = [
        Limit::NodeCount,
        Limit::ContainmentDepth,
        Limit::EdgeCountPerNode,
        Limit::MutationCountPerPatch,
        Limit::RoleCount,
        Limit::KindCount,
        Limit::ChoiceCountPerDecision,
        Limit::ChecklistItemCountPerNode,
        Limit::IdentityCountPerFill,
        Limit::ConditionDepth,
        Limit::ConditionClauseCount,
        Limit::IdBytes,
        Limit::TitleBytes,
        Limit::BodyBytes,
        Limit::LinkBytes,
        Limit::EmailBytes,
        Limit::ReasonBytes,
        Limit::EmailCountPerIdentity,
        Limit::AliasCountPerIdentity,
        Limit::IdentityCountPerDeployment,
        Limit::JourneyCountPerMerge,
        Limit::ResourceCountPerNode,
        Limit::NoteCountPerNode,
        Limit::LinkCountPerNode,
        Limit::OffsetDays,
        Limit::Weight,
        Limit::GraphBytes,
        Limit::RequestBytes,
        Limit::ChainCountPerRejection,
        Limit::ExplanationEntryCount,
    ];

    /// The limit's value.
    #[must_use]
    pub const fn max(self) -> u32 {
        match self {
            Limit::NodeCount => NODE_COUNT_MAX,
            Limit::ContainmentDepth => CONTAINMENT_DEPTH_MAX,
            Limit::EdgeCountPerNode => EDGE_COUNT_PER_NODE_MAX,
            Limit::MutationCountPerPatch => MUTATION_COUNT_PER_PATCH_MAX,
            Limit::RoleCount => ROLE_COUNT_MAX,
            Limit::KindCount => KIND_COUNT_MAX,
            Limit::ChoiceCountPerDecision => CHOICE_COUNT_PER_DECISION_MAX,
            Limit::ChecklistItemCountPerNode => CHECKLIST_ITEM_COUNT_PER_NODE_MAX,
            Limit::IdentityCountPerFill => IDENTITY_COUNT_PER_FILL_MAX,
            Limit::ConditionDepth => CONDITION_DEPTH_MAX,
            Limit::ConditionClauseCount => CONDITION_CLAUSE_COUNT_MAX,
            Limit::IdBytes => ID_BYTES_MAX,
            Limit::TitleBytes => TITLE_BYTES_MAX,
            Limit::BodyBytes => BODY_BYTES_MAX,
            Limit::LinkBytes => LINK_BYTES_MAX,
            Limit::EmailBytes => EMAIL_BYTES_MAX,
            Limit::ReasonBytes => REASON_BYTES_MAX,
            Limit::EmailCountPerIdentity => EMAIL_COUNT_PER_IDENTITY_MAX,
            Limit::AliasCountPerIdentity => ALIAS_COUNT_PER_IDENTITY_MAX,
            Limit::IdentityCountPerDeployment => IDENTITY_COUNT_PER_DEPLOYMENT_MAX,
            Limit::JourneyCountPerMerge => JOURNEY_COUNT_PER_MERGE_MAX,
            Limit::ResourceCountPerNode => RESOURCE_COUNT_PER_NODE_MAX,
            Limit::NoteCountPerNode => NOTE_COUNT_PER_NODE_MAX,
            Limit::LinkCountPerNode => LINK_COUNT_PER_NODE_MAX,
            Limit::OffsetDays => OFFSET_DAYS_MAX,
            Limit::Weight => WEIGHT_MAX,
            Limit::GraphBytes => GRAPH_BYTES_MAX,
            Limit::RequestBytes => REQUEST_BYTES_MAX,
            Limit::ChainCountPerRejection => CHAIN_COUNT_PER_REJECTION_MAX,
            Limit::ExplanationEntryCount => EXPLANATION_ENTRY_COUNT_MAX,
        }
    }

    /// The limit's name, as errors print it: the constant's name in lower case.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Limit::NodeCount => "node_count_max",
            Limit::ContainmentDepth => "containment_depth_max",
            Limit::EdgeCountPerNode => "edge_count_per_node_max",
            Limit::MutationCountPerPatch => "mutation_count_per_patch_max",
            Limit::RoleCount => "role_count_max",
            Limit::KindCount => "kind_count_max",
            Limit::ChoiceCountPerDecision => "choice_count_per_decision_max",
            Limit::ChecklistItemCountPerNode => "checklist_item_count_per_node_max",
            Limit::IdentityCountPerFill => "identity_count_per_fill_max",
            Limit::ConditionDepth => "condition_depth_max",
            Limit::ConditionClauseCount => "condition_clause_count_max",
            Limit::IdBytes => "id_bytes_max",
            Limit::TitleBytes => "title_bytes_max",
            Limit::BodyBytes => "body_bytes_max",
            Limit::LinkBytes => "link_bytes_max",
            Limit::EmailBytes => "email_bytes_max",
            Limit::ReasonBytes => "reason_bytes_max",
            Limit::EmailCountPerIdentity => "email_count_per_identity_max",
            Limit::AliasCountPerIdentity => "alias_count_per_identity_max",
            Limit::IdentityCountPerDeployment => "identity_count_per_deployment_max",
            Limit::JourneyCountPerMerge => "journey_count_per_merge_max",
            Limit::ResourceCountPerNode => "resource_count_per_node_max",
            Limit::NoteCountPerNode => "note_count_per_node_max",
            Limit::LinkCountPerNode => "link_count_per_node_max",
            Limit::OffsetDays => "offset_days_max",
            Limit::Weight => "weight_max",
            Limit::GraphBytes => "graph_bytes_max",
            Limit::RequestBytes => "request_bytes_max",
            Limit::ChainCountPerRejection => "chain_count_per_rejection_max",
            Limit::ExplanationEntryCount => "explanation_entry_count_max",
        }
    }

    /// Checks `count` against the limit.
    ///
    /// # Errors
    ///
    /// [`LimitExceeded`] when `count` is past the limit.
    pub fn check(self, count: usize) -> Result<(), LimitExceeded> {
        let count = u64::try_from(count).unwrap_or(u64::MAX);
        if count > u64::from(self.max()) {
            return Err(LimitExceeded { limit: self, count });
        }
        Ok(())
    }
}

impl Limit {
    /// The name, for the string serde form.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        self.name()
    }
}

impl std::str::FromStr for Limit {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        Limit::ALL
            .into_iter()
            .find(|limit| limit.name() == text)
            .ok_or_else(|| format!("{text:?} is not a limit"))
    }
}

// PRACTICES: Explicit limits.
crate::serde_util::string_serde!(Limit, "Limit", "A limit, by name.");

/// A value past one of its limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimitExceeded {
    /// The limit exceeded.
    pub limit: Limit,
    /// The count, size, or value that exceeded it.
    pub count: u64,
}

impl fmt::Display for LimitExceeded {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} is more than the {} allowed ({})",
            self.count,
            self.limit.max(),
            self.limit.name()
        )
    }
}

impl LimitExceeded {
    /// The excess as a person reads it, about `what` (a phrase that starts a sentence, such as
    /// "The title"): "The title is 300 bytes; at most 256."
    #[must_use]
    pub fn words(&self, what: &str) -> String {
        let (count, max) = (self.count, self.limit.max());
        match self.limit {
            Limit::IdBytes
            | Limit::TitleBytes
            | Limit::BodyBytes
            | Limit::LinkBytes
            | Limit::EmailBytes
            | Limit::ReasonBytes
            | Limit::GraphBytes
            | Limit::RequestBytes => {
                format!("{what} is {count} bytes; at most {max}. Shorten it.")
            }
            Limit::OffsetDays | Limit::Weight => format!("{what} is {count}; at most {max}."),
            _ => format!("{what} has {count}; at most {max}."),
        }
    }
}

impl std::error::Error for LimitExceeded {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_limit_accepts_its_value_and_rejects_one_more() {
        for limit in Limit::ALL {
            let at = usize::try_from(limit.max()).unwrap();
            assert_eq!(limit.check(at), Ok(()), "{limit}");
            let past = limit.check(at + 1).unwrap_err();
            assert_eq!(past.limit, limit);
            assert!(past.to_string().contains(limit.name()));
        }
    }

    #[test]
    fn limit_names_are_distinct() {
        let names: std::collections::BTreeSet<_> = Limit::ALL.iter().map(|l| l.name()).collect();
        assert_eq!(names.len(), Limit::ALL.len());
    }
}
