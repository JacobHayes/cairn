//! Rejections (A15, H5; PRACTICES, Errors, panics, and rejections): the engine's value-typed
//! answer to a patch it will not apply, listing every violation by path. Serialized
//! unchanged through the API and MCP.

use std::collections::BTreeSet;
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::chain::ChainList;
use crate::collections::{BoundedVec, ByDocumentSize, CollectionError};
use crate::domain::Domain;
use crate::event::Subject;
use crate::field::NodeField;
use crate::id::{PatchId, Path, ProposalId};
use crate::limits::Limit;
use crate::number::Revision;
use crate::state::{Guard, GuardFailure};
use crate::touched::TouchedSet;

/// What a violation breaks (PRD Invariants, D1, D4, A11, A18, A19, B6, E3, E6, H3).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum ViolationCode {
    // Identity and references.
    DuplicateSiblingId,
    DuplicateKey,
    RetiredKeyReused,
    UnresolvedReference,
    WrongReferenceKind,
    // Containment and dependencies.
    ContainmentCycle,
    LeafWithChildren,
    DependencyCycle,
    EdgeToAncestorOrDescendant,
    RequiresDuplicatesCondition,
    // Dates (F5).
    ContradictoryChain,
    // Conditions, stages, roles, kinds, flags.
    ConditionAnswerTypeMismatch,
    ConditionOnOwnSubtree,
    StageBoundNotMilestone,
    UndeclaredKind,
    SingleKindOnMultiRole,
    FillsRoleCardinality,
    SeveralFillingDecisions,
    FeedsMilestoneNotMilestone,
    SeveralFeedingDecisions,
    SeveralFinalMilestones,
    FieldNotOnKind,
    StateNotOnKind,
    LimitExceeded,
    // Removal (A18).
    DanglingReference,
    RemovalWidened,
    StillReferenced,
    // Journey state.
    AnswerTypeMismatch,
    EntityUnresolved,
    IllegalTransition,
    ReasonRequired,
    GuardFailed,
    NotRelevant,
    SnoozeOnSelf,
    SnoozeCycle,
    SnoozeNotActionable,
    SnoozedThroughContainer,
    FilledThroughDecision,
    PinnedThroughDecision,
    // Domains and lifecycle.
    MutationNotForTarget,
    LineageInvalid,
    ArchivedJourney,
    TargetExists,
    TargetMissing,
    DeletedJourneyId,
    DraftExists,
    NoDraft,
    VersionInUse,
    // Entities (E6, H3).
    EntityKeyTaken,
    EmailTaken,
    AliasCycle,
    MergeBreaksJourney,
    // Proposals (I6, C14).
    ProposalNotOpen,
    UnresolvedReviewItem,
}

/// Where a violation is (A15: listed by path; PRD Non-functional, Config-first: specific
/// enough to fix without reading source).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Location {
    /// The mutation's position in the patch, when one mutation caused it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mutation: Option<u32>,
    /// The key it is about.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<Subject>,
    /// The node's path in the graph the patch would produce.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<Path>,
    /// The node field involved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<NodeField>,
}

/// One violation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Violation {
    /// What it breaks.
    pub code: ViolationCode,
    /// Where.
    pub at: Location,
    /// A sentence for people, generated for display.
    pub message: String,
    /// Other keys involved: a cycle's members, the other holder of a duplicate.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<Subject>,
    /// The limit exceeded, for `limit_exceeded`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<Limit>,
    /// The guard a bypass would accept this past, when it is a bypassable guard failure
    /// (D4: `deps_done`, `has_artifact`, `broken_down`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bypassable: Option<Guard>,
    /// The specific guard failures present (D4).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub failures: BTreeSet<GuardFailure>,
    /// The contradictory chains, for `contradictory_chain` (F5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chains: Option<ChainList>,
    /// The patch's other mutations that brought it about, beside `at.mutation` (D4: a
    /// completion and the mutation that gave it an unfinished dependency in the same patch).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub caused_by: BTreeSet<u32>,
}

/// A violation list with at least one entry (a rejection always says why).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    try_from = "BoundedVec<Violation, ByDocumentSize>",
    into = "BoundedVec<Violation, ByDocumentSize>"
)]
#[schemars(with = "BoundedVec<Violation, ByDocumentSize>")]
pub struct Violations(BoundedVec<Violation, ByDocumentSize>);

impl Violations {
    /// Collects at least one violation.
    ///
    /// # Errors
    ///
    /// When there are none.
    pub fn new(violations: Vec<Violation>) -> Result<Self, CollectionError> {
        Self::try_from(BoundedVec::new(violations)?)
    }

    /// The violations.
    #[must_use]
    pub fn as_slice(&self) -> &[Violation] {
        self.0.as_slice()
    }
}

impl TryFrom<BoundedVec<Violation, ByDocumentSize>> for Violations {
    type Error = CollectionError;

    fn try_from(
        violations: BoundedVec<Violation, ByDocumentSize>,
    ) -> Result<Self, CollectionError> {
        if violations.is_empty() {
            return Err(CollectionError::Empty);
        }
        Ok(Self(violations))
    }
}

impl From<Violations> for BoundedVec<Violation, ByDocumentSize> {
    fn from(violations: Violations) -> Self {
        violations.0
    }
}

/// Which revision a stale patch named wrongly (H5).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RevisionOf {
    /// A domain's revision.
    Domain(Domain),
    /// A proposal's editing revision.
    Proposal(ProposalId),
}

/// One revision a patch named that has moved on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RevisionConflict {
    /// Whose revision.
    pub of: RevisionOf,
    /// The revision the patch named.
    pub expected: Revision,
    /// The revision now.
    pub current: Revision,
}

/// Why a patch was not applied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "rejection", rename_all = "snake_case", deny_unknown_fields)]
pub enum Rejection {
    /// A revision the patch named has advanced (H5). The touched set of the intervening
    /// events lets the client retry on its own when it does not overlap the patch's.
    Stale {
        /// The revisions that moved on.
        conflicts: Vec<RevisionConflict>,
        /// What the intervening events touched.
        intervening: TouchedSet,
    },
    /// The patch id was committed before with different content (H5).
    PatchIdReused {
        /// The patch id.
        patch_id: PatchId,
    },
    /// The patch would break invariants or guards: every violation (A15).
    Invalid {
        /// The violations.
        violations: Violations,
    },
}

impl fmt::Display for ViolationCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match serde_json::to_value(self) {
            Ok(serde_json::Value::String(name)) => formatter.write_str(&name),
            _ => write!(formatter, "{self:?}"),
        }
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.at.path {
            Some(path) => write!(formatter, "{path}: {} ({})", self.message, self.code),
            None => write!(formatter, "{} ({})", self.message, self.code),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn chain() -> serde_json::Value {
        json!({"chain": {"constraints": [{"before": "created_at", "after": {"node": {"node": "n_a", "point": "start"}}, "offset_days": 3, "source": {"estimate": {"node": "n_a"}}}]}, "shortfall_days": 2})
    }

    #[test]
    fn contradictory_chains_at_and_past_their_limit() {
        let at = json!({"chains": vec![chain(); 16], "more": true});
        assert!(serde_json::from_value::<ChainList>(at).is_ok());
        let past = json!({"chains": vec![chain(); 17]});
        let error = serde_json::from_value::<ChainList>(past).unwrap_err();
        assert!(
            error
                .to_string()
                .contains(Limit::ChainCountPerRejection.name()),
            "{error}"
        );
    }

    #[test]
    fn an_invalid_rejection_lists_at_least_one_violation() {
        let violation = json!({"code": "dependency_cycle", "at": {"path": "setup/plan"}, "message": "The plan requires itself."});
        let rejection: Rejection = serde_json::from_value(
            json!({"rejection": "invalid", "violations": [violation.clone(), violation]}),
        )
        .unwrap();
        let Rejection::Invalid { violations } = rejection else {
            panic!("an invalid rejection")
        };
        assert_eq!(violations.as_slice().len(), 2);
        assert!(
            violations.as_slice()[0]
                .to_string()
                .starts_with("setup/plan")
        );
        assert!(
            serde_json::from_value::<Rejection>(json!({"rejection": "invalid", "violations": []}))
                .is_err()
        );
    }
}
