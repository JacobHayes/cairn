//! Constraint chains (F2, F3, F5, F6): the one shape every date relationship takes, "B is
//! at least k days after A", and the chains of them that explain a bound or make a plan
//! impossible. The chain that explains a due date and the chain that makes a plan
//! contradictory are the same kind of thing, so both use these types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::collections::{BoundedVec, ChainCountPerRejection};
use crate::id::NodeKey;
use jiff::civil::Date;

/// Which instant of a node (F2): work has a start and a finish; a decision or milestone has
/// one instant, which is both.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum InstantPoint {
    /// When it starts.
    Start,
    /// When it finishes.
    Finish,
}

/// A point in time the date network solves for (ARCHITECTURE, Date network). A container's
/// entry is internal and never shown, so it has no form here.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Instant {
    /// A node's start or finish.
    Node {
        /// The node.
        node: NodeKey,
        /// Which instant.
        point: InstantPoint,
    },
    /// The journey's `created_at`.
    CreatedAt,
}

/// How a dependency arose (PRD glossary, Condition gate / implicit edge).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum DependencyVia {
    /// An explicit `requires` edge.
    Explicit,
    /// A parent waiting on its child (Containment).
    Containment,
    /// An ancestor's requirement, inherited.
    Inherited {
        /// The ancestor whose requirement applies.
        ancestor: NodeKey,
    },
    /// A condition referencing a decision (Gating).
    Condition {
        /// The node whose condition creates the gate.
        condition_on: NodeKey,
    },
    /// A stage's opening milestone (F4).
    StageOpening {
        /// The stage.
        group: NodeKey,
    },
}

/// Where a constraint comes from (F2): what to edit to change it (F7).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ConstraintSource {
    /// A node's `due_by` rule.
    DueBy {
        /// The node.
        node: NodeKey,
    },
    /// A node's `not_before` rule.
    NotBefore {
        /// The node.
        node: NodeKey,
    },
    /// A dependency: the dependent starts after the requirement finishes.
    Dependency {
        /// The dependent.
        node: NodeKey,
        /// The requirement.
        requires: NodeKey,
        /// How the dependency arose.
        via: DependencyVia,
    },
    /// An estimate: a node finishes at least its estimate after it starts.
    Estimate {
        /// The node.
        node: NodeKey,
    },
    /// Containment: a child finishes before its parent's own work starts.
    Containment {
        /// The parent.
        parent: NodeKey,
        /// The child.
        child: NodeKey,
    },
    /// A stage's closing milestone bounding the group's finish (F4).
    StageClose {
        /// The stage.
        group: NodeKey,
    },
}

/// One constraint: `after` is at least `offset_days` after `before` (offsets may be zero or
/// negative).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Constraint {
    /// The earlier instant.
    pub before: Instant,
    /// The later instant.
    pub after: Instant,
    /// The least number of days between them.
    pub offset_days: i32,
    /// Where it comes from.
    pub source: ConstraintSource,
    /// It passes through an undecided node, so it may not apply (F2).
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub conditional: bool,
}

/// What fixes an instant on a chain: a pin, or an actual date (F1, F6).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FixedBy {
    /// A pin, which a resolution may move (F5).
    Pin,
    /// An actual date, a fact that is never moved (F6).
    Actual,
}

/// A date fixed on a chain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FixedDate {
    /// The instant.
    pub instant: Instant,
    /// The date.
    pub date: Date,
    /// A pin or an actual.
    pub fixed_by: FixedBy,
}

/// A chain of constraints, with the dates fixed on it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Chain {
    /// The constraints, in order along the chain.
    pub constraints: Vec<Constraint>,
    /// The pins and actuals on it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixed: Vec<FixedDate>,
}

/// A chain that requires a date to precede itself (F5), or a plan reality can no longer
/// meet (F6), with how many days it is short.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShortChain {
    /// The chain.
    pub chain: Chain,
    /// Days short.
    pub shortfall_days: u32,
}

/// The contradictory chains a rejection lists (F5): at most
/// `chain_count_per_rejection_max`, and whether there were more.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChainList {
    /// The chains found, up to the limit.
    pub chains: BoundedVec<ShortChain, ChainCountPerRejection>,
    /// The search was cut short at the limit.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub more: bool,
}
