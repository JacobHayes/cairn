//! Derived shapes as they cross a crate or the API (D3, D7, F7; ARCHITECTURE, Read path):
//! `Derived` as derive returns it, its explanations, consequences, and the derive inputs a
//! host supplies. Types passed between derive passes stay in the engine. Nothing here is
//! ever stored (D6, J1).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::chain::{Chain, ConstraintSource, DependencyVia, ShortChain};
use crate::collections::{BoundedVec, ByDocumentSize, CollectionError};
use crate::domain::{Deployment, Journey};
use crate::id::{IdentityKey, KindKey, NodeKey, RoleKey};
use crate::limits::{EXPLANATION_ENTRY_COUNT_MAX, Limit, WEIGHT_MAX};
use crate::node::NodeKind;
use crate::state::{Guard, GuardFailure, SnoozeTarget};
use jiff::civil::Date;

/// A node's relevance (Gating): three-valued.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Relevance {
    /// Applies.
    Relevant,
    /// Does not apply.
    NotRelevant,
    /// Depends on a decision still open and relevant.
    Undecided,
}

// PRD: C8.
/// A relevance value and what produced it (node detail names the ancestor or decision).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RelevanceExplanation {
    /// The value.
    pub value: Relevance,
    /// The node whose condition decided it, when not the node itself; none when no
    /// condition applies.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition_on: Option<NodeKey>,
    /// The decisions that condition reads.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub decisions: BTreeSet<NodeKey>,
    /// A force include made it relevant.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub forced: bool,
    // PRD: D8.
    /// When the value is `not_relevant` only because a decision it reads is itself
    /// undecided (so unanswered, and every value operator on it false), the undecided
    /// decisions it rests on: reading them as still to come would leave the node undecided.
    /// Empty when the value is settled. The value itself is unchanged, so gravity, unlocks
    /// and blocking follow Gating exactly; display state shows such a node as `conditional`.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub pending_on: BTreeSet<NodeKey>,
    // PRD: D4.
    /// For an undecided value, the decisions that make it undecided and can be
    /// answered now: open, relevant, and not under a skip, read by a condition leaving it
    /// undecided, its own or an ancestor's, up to a force include. Empty for any other
    /// value (narrower than every decision still unanswered).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub unanswered: BTreeSet<NodeKey>,
}

// PRD: D8, D3.
/// The one state every surface shows for a node, composed from relevance, stored state,
/// effective skip, auto-done, blocking and snooze, and from the derived flags, which it leaves in
/// place. Stored state is what transitions act on; this is what people and agents read. The
/// first matching value in this order wins: `not_relevant`, `skipped`, `done`, `snoozed`,
/// `active`, `conditional`, `blocked`, `scheduled`, `ready`.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DisplayState {
    /// Nothing is in the way and nothing has started: actionable work, a container waiting
    /// only on its own children with nothing started, or one ready to finish.
    Ready,
    /// Started: stored `active`, or a container whose children are the only thing it waits
    /// on and work beneath it has started (a deliverable or action under it is active or done).
    Active,
    /// In scope and unfinished, and its own or an inherited gate is unsatisfied. A container
    /// is never blocked by its own children.
    Blocked,
    /// It may apply: relevance is `undecided` and it is unfinished, or relevance is
    /// `not_relevant` but pending on an undecided decision (`pending_on`).
    Conditional,
    /// Unblocked and held for a date ahead: an `auto_done` milestone whose effective date is
    /// ahead, another milestone whose pinned date is ahead, or unstarted work or a decision
    /// whose own not-before date is ahead.
    Scheduled,
    /// A snooze holds.
    Snoozed,
    /// `done`, `decided`, a milestone done by its date, or a group that satisfies dependencies.
    Done,
    // PRD: D1a.
    /// Stored `skipped`, or effectively skipped.
    Skipped,
    /// Does not apply, and settled (Gating): the recorded state survives beneath it.
    NotRelevant,
}

impl DisplayState {
    /// Every display state, in the order a count lists them: what is moving first, what is
    /// out last.
    pub const ALL: [DisplayState; 9] = [
        DisplayState::Active,
        DisplayState::Ready,
        DisplayState::Blocked,
        DisplayState::Conditional,
        DisplayState::Scheduled,
        DisplayState::Snoozed,
        DisplayState::Done,
        DisplayState::Skipped,
        DisplayState::NotRelevant,
    ];

    /// The word the state is said by, to a person or by an agent; for a node of `kind`, the
    /// kind's own word where it says the state differently (a decision is to decide, then
    /// decided; a group that has not started says so).
    #[must_use]
    pub const fn word(self, kind: Option<NodeKind>) -> &'static str {
        match (self, kind) {
            (DisplayState::Ready, Some(NodeKind::Decision)) => "to decide",
            (DisplayState::Done, Some(NodeKind::Decision)) => "decided",
            (DisplayState::Ready, Some(NodeKind::Group)) => "not started",
            (DisplayState::Ready, _) => "ready",
            (DisplayState::Active, _) => "active",
            (DisplayState::Blocked, _) => "blocked",
            (DisplayState::Conditional, _) => "conditional",
            (DisplayState::Scheduled, _) => "scheduled",
            (DisplayState::Snoozed, _) => "snoozed",
            (DisplayState::Done, _) => "done",
            (DisplayState::Skipped, _) => "skipped",
            (DisplayState::NotRelevant, _) => "not relevant",
        }
    }
}

// PRD: D1.
/// A dependency that blocks a node (Gating: Blocked).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Blocker {
    /// The unsatisfied dependency.
    pub node: NodeKey,
    /// How the dependency arose.
    pub via: DependencyVia,
    // PRD: D8.
    /// The date the dependency is scheduled for, while it is held for a date ahead, so what
    /// waits on it can say when.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<Date>,
}

/// A derived weight sum (Priority: gravity, unlocks, and their contributions) in millionths
/// of a weight, written as its value in weight units (`2.5`). A weight is a whole number, and
/// the undecided discount and the other-owner factor are counted in thousandths, so each term
/// (weight x discount x factor) is a whole number of millionths: sums are exact, and at the
/// limits (`node_count_max` x `weight_max` x the largest discount and factor, 2 x 10^15) they
/// stay below [`Score::MAX`], where a value written as a JSON number reads back exactly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Score(u64);

impl Score {
    /// Millionths in one weight unit.
    pub const UNIT: u64 = 1_000_000;
    /// The largest score written as its value that reads back exactly (2^51 millionths): the
    /// float nearest a value is within 2^-21 of it, so scaling it back is off by under half a
    /// millionth.
    pub const MAX: Score = Score(1 << 51);

    /// A score of `millionths` millionths of a weight.
    #[must_use]
    pub const fn from_millionths(millionths: u64) -> Self {
        Self(millionths)
    }

    /// The score in millionths of a weight.
    #[must_use]
    pub const fn millionths(self) -> u64 {
        self.0
    }

    /// The value in weight units, for display and the rank's float terms.
    #[must_use]
    pub fn value(self) -> f64 {
        // At most 2^51 (asserted where scores are summed), so the conversion is exact.
        #[allow(clippy::cast_precision_loss)]
        let millionths = self.0 as f64;
        millionths / 1_000_000.0
    }
}

impl Serialize for Score {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.value())
    }
}

impl<'de> Deserialize<'de> for Score {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = f64::deserialize(deserializer)?;
        let millionths = whole(value * 1_000_000.0, Score::MAX.millionths()).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "{value} is not a non-negative multiple of 0.000001 within 2^51 millionths"
            ))
        })?;
        Ok(Self(millionths))
    }
}

impl JsonSchema for Score {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Score".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "number", "minimum": 0 })
    }
}

/// The whole number `scaled` is, when it is one within float error, non-negative, and at most
/// `max`.
fn whole(scaled: f64, max: u64) -> Option<u64> {
    let rounded = scaled.round();
    // Float error only: scaling a short decimal like 0.1 is off by about 1e-14, and a large
    // value's nearest float by up to a quarter at 2^51 (about 2.2e-16 of it).
    let close = (scaled - rounded).abs() <= 1e-9 + scaled.abs() * 2.3e-16;
    #[allow(clippy::cast_precision_loss)]
    let within = scaled >= 0.0 && rounded <= max as f64;
    if !(scaled.is_finite() && close && within) {
        return None;
    }
    // Checked above: a whole, non-negative number within `max`.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some(rounded as u64)
}

/// A rank constant counted in thousandths between `MIN` and `MAX` thousandths, written as
/// its value (`0.5`): the undecided discount and the other-owner factor, which weights are
/// multiplied by exactly (Priority).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Thousandths<const MIN: u32, const MAX: u32>(u32);

/// The undecided discount (Priority: `effective_weight`), from 0 to 1.
pub type UndecidedDiscount = Thousandths<0, 1_000>;

/// The factor a target owned by someone else gets in unlocks (Priority: `owner_factor`), from 1 to
/// `weight_max`: the limits table names no bound for it, so it takes the nearest named one.
pub type OwnerFactor = Thousandths<1_000, { WEIGHT_MAX * 1_000 }>;

impl<const MIN: u32, const MAX: u32> Thousandths<MIN, MAX> {
    /// The value in thousandths.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl<const MIN: u32, const MAX: u32> TryFrom<u32> for Thousandths<MIN, MAX> {
    type Error = String;

    fn try_from(thousandths: u32) -> Result<Self, String> {
        if (MIN..=MAX).contains(&thousandths) {
            Ok(Self(thousandths))
        } else {
            Err(format!(
                "{thousandths} thousandths is outside {MIN} to {MAX} thousandths"
            ))
        }
    }
}

impl<const MIN: u32, const MAX: u32> Serialize for Thousandths<MIN, MAX> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(f64::from(self.0) / 1_000.0)
    }
}

impl<'de, const MIN: u32, const MAX: u32> Deserialize<'de> for Thousandths<MIN, MAX> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = f64::deserialize(deserializer)?;
        let thousandths = whole(value * 1_000.0, u64::from(MAX))
            .and_then(|whole| u32::try_from(whole).ok())
            .ok_or_else(|| {
                serde::de::Error::custom(format!(
                    "{value} is not a multiple of 0.001 from {MIN} to {MAX} thousandths"
                ))
            })?;
        Self::try_from(thousandths).map_err(serde::de::Error::custom)
    }
}

impl<const MIN: u32, const MAX: u32> JsonSchema for Thousandths<MIN, MAX> {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        format!("Thousandths{MIN}To{MAX}").into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let minimum = f64::from(MIN) / 1_000.0;
        let maximum = f64::from(MAX) / 1_000.0;
        schemars::json_schema!({ "type": "number", "minimum": minimum, "maximum": maximum, "multipleOf": 0.001 })
    }
}

// ARCHITECTURE: Read path.
/// A list of explanation entries with its total: complete inside
/// the engine and the browser; a server response keeps the largest entries up to
/// `explanation_entry_count_max` and the total.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    deny_unknown_fields,
    bound(serialize = "T: Serialize", deserialize = "T: Deserialize<'de>")
)]
#[schemars(rename = "Explained{T}")]
pub struct Explained<T> {
    /// The entries, largest first.
    pub entries: BoundedVec<T, ByDocumentSize>,
    /// How many there are in all.
    pub total: u32,
}

impl<T> Default for Explained<T> {
    fn default() -> Self {
        Self {
            entries: BoundedVec::default(),
            total: 0,
        }
    }
}

impl<T> Explained<T> {
    /// A complete list.
    ///
    /// # Errors
    ///
    /// When the list is past the document cap.
    pub fn complete(entries: Vec<T>) -> Result<Self, CollectionError> {
        let total = u32::try_from(entries.len()).unwrap_or(u32::MAX);
        Ok(Self {
            entries: BoundedVec::new(entries)?,
            total,
        })
    }

    /// The list a server response carries: the first `explanation_entry_count_max` entries
    /// (the largest, as the engine orders them) and the unchanged total.
    ///
    /// # Panics
    ///
    /// Never: truncating to the limit always fits it.
    #[must_use]
    pub fn for_response(self) -> Self {
        let limit = usize::try_from(EXPLANATION_ENTRY_COUNT_MAX).unwrap_or(usize::MAX);
        let mut entries = self.entries.into_vec();
        entries.truncate(limit);
        assert!(Limit::ExplanationEntryCount.check(entries.len()).is_ok());
        Self {
            entries: BoundedVec::new(entries).unwrap_or_default(),
            total: self.total,
        }
    }
}

/// A finite, non-negative real: a rank or a rank coefficient. Rejects NaN and infinities on
/// parse, which JSON cannot carry, so a value read from YAML always survives the JSON wire.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Real(f64);

impl Real {
    /// The value.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for Real {
    type Error = String;

    fn try_from(value: f64) -> Result<Self, String> {
        if value.is_finite() && value >= 0.0 {
            Ok(Self(value))
        } else {
            Err(format!("{value} is not a finite, non-negative number"))
        }
    }
}

impl Serialize for Real {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.0)
    }
}

impl<'de> Deserialize<'de> for Real {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_from(f64::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for Real {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Real".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "number", "minimum": 0 })
    }
}

/// A gravity or unlocks contribution: the node and how much it adds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Contribution {
    /// The contributing node.
    pub node: NodeKey,
    /// What it adds.
    pub score: Score,
    /// It is owned by someone other than the node's owner (the owner factor of unlocks).
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub other_owner: bool,
}

// PRD: F3, F7.
/// A derived date bound and the chain that produced it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Bound {
    /// The date.
    pub date: Date,
    /// The chain from a pin, actual, or today that produced it.
    pub chain: Chain,
}

// PRD: F1, F7.
/// Where a milestone's effective date comes from (pin, actual, and derived dates are
/// told apart).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DateOrigin {
    /// The date it was done.
    Actual,
    // PRD: E3.
    /// Its pin, set directly or by answering its feeding decision.
    Pin,
    // PRD: F3.
    /// Its derived due date.
    Due,
}

// PRD: F1.
/// A milestone's effective date: its actual date if done, else its pin, else its
/// derived due. What other constraints see when they reference it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EffectiveDate {
    /// The date.
    pub date: Date,
    /// Where it comes from.
    pub origin: DateOrigin,
}

// PRD: F3, F6.
/// A node's derived dates. Every bound is null when nothing reaches it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NodeDates {
    /// The earliest it can start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub earliest_start: Option<Bound>,
    /// The latest it can start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_start: Option<Bound>,
    /// The latest it can finish.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<Bound>,
    /// Latest start minus today; null is "no deadline".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slack_days: Option<i32>,
    // PRD: F6, D3.
    /// How many days its due is before today, for an overdue node; none when it is
    /// not overdue.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overdue_days: Option<u32>,
    // PRD: F6.
    /// The plan can no longer be met.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortfall: Option<ShortChain>,
    // PRD: F1.
    /// A milestone's effective date; none for other kinds, or when nothing gives one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_date: Option<EffectiveDate>,
    // PRD: A8, D8.
    /// The earliest its own not-before rules let it start: the latest of each source's
    /// earliest date plus the rule's offset. Work or a decision whose date this is, while it
    /// is after today, is scheduled and must wait for it to finish without a bypass. None
    /// when it has no such rule, or no source has a date.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_before: Option<Date>,
    // PRD: F6, D8.
    /// Finished before something it was finished ahead of on purpose, on today's dates: a
    /// dependency or its own date its guard bypass accepted, a stage opening that does not
    /// gate, or a milestone's own not-before rule. None when none of those would have put
    /// its finish later. Other shortfalls still apply.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done_early: Option<DoneEarly>,
}

// PRD: F6, D8.
/// How early a node finished, and ahead of what.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DoneEarly {
    /// Days between its finish and the date it was finished ahead of.
    pub days: u32,
    /// The date it was finished ahead of: the requirement's or opening's earliest date today
    /// (its recorded date once it has one; a milestone's date as shown, its actual, pin or
    /// due, when that is later), or its own not-before date.
    pub date: Date,
    /// What it was finished ahead of: a dependency, with how it arose (a stage opening
    /// among them), or its own not-before rule.
    pub source: ConstraintSource,
}

// PRD: E2.
/// Where a node's effective participation of one kind comes from (in order: explicit identities, then the role, then inherited).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ParticipationOrigin {
    /// Explicit identities on the node.
    Explicit,
    /// The node's own role reference.
    Role(RoleKey),
    /// The nearest declaring ancestor.
    Ancestor(NodeKey),
    /// The graph's `default_owner` (owner only).
    DefaultOwner(RoleKey),
}

// PRD: E2.
/// A node's effective participation of one kind.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EffectiveParticipation {
    /// The identities, resolved through aliases.
    pub identities: BTreeSet<IdentityKey>,
    /// Where it comes from.
    pub origin: ParticipationOrigin,
}

// PRD: C2, C4.
/// A container's progress over the work beneath it, counted over every proper
/// descendant that is an item: not a group, in scope (relevant or undecided), and neither
/// skipped nor skipped through a container. A kept node under a skipped container, and a
/// force-included node under a not-relevant one, count on their own terms. Derived on every
/// read in one pass over the tree, O(nodes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    /// Items whose display state is `done` (finished, decided, or done by its date).
    pub done: u32,
    /// Items.
    pub total: u32,
    // PRD: D3.
    /// Items that are overdue.
    pub overdue: u32,
    // PRD: D2.
    /// Decisions among the items that are actionable now, snoozed ones included.
    pub decisions_needed: u32,
}

// PRD: D3.
/// Every derived value for one node, with the inputs that explain it. The flags are its own,
/// each independent of the others, so they stay booleans rather than a state enum.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NodeDerived {
    /// Relevance and what produced it.
    pub relevance: RelevanceExplanation,
    // PRD: D1a.
    /// Skipped through an ancestor's skip.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub effectively_skipped: bool,
    /// What blocks it itself: its own explicit requirements, condition gates, and stage
    /// opening, and its children, each unsatisfied. What it inherits is listed once, on the
    /// ancestor that holds it (`blocked_through`), never copied onto each descendant. A node
    /// that is not blocked lists what its own requirements, conditions, and opening still
    /// hold back beneath it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_by: Vec<Blocker>,
    /// The ancestors, nearest first, whose own unsatisfied requirements, openings, or
    /// conditions block it through containment; each lists them in its own `blocked_by`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_through: Vec<NodeKey>,
    // PRD: D8.
    /// The state every surface shows for the node, composed from relevance, stored state
    /// and the flags beside it, which stay as they are.
    pub display_state: DisplayState,
    // PRD: D2.
    /// Relevant, not blocked, and non-terminal.
    pub actionable: bool,
    // PRD: E1.
    /// No owner, for a non-group node that is in scope and unfinished; a not-relevant or
    /// finished node, or a group, never is.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub unassigned: bool,
    /// Effective participations by kind.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub participations: BTreeMap<KindKey, EffectiveParticipation>,
    // PRD: B10.
    /// Its seeding identity left the role it was broken down by.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub membership_lost: bool,
    // PRD: D4.
    /// Why a terminal node's completing guards would now fail.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub stale: BTreeSet<GuardFailure>,
    /// Non-terminal with its due before today.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub overdue: bool,
    /// Dates.
    pub dates: NodeDates,
    // PRD: C2, C4.
    /// A container's progress over the work beneath it; none for a node with no
    /// children.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<Progress>,
    // PRD: D4.
    /// The bypassable guards its completing transition (complete a deliverable or
    /// action, answer an open decision, complete a pending milestone) would fail if attempted
    /// now with no bypass in the patch. Empty for every node with no such transition left:
    /// finished, not relevant, or a group.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub failing_guards: BTreeSet<Guard>,
    // PRD: D8.
    /// Stored `active` while its own or an inherited gate is unsatisfied; a container's
    /// children are not a gate.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub started_early: bool,
    // PRD: D8.
    /// The unsatisfied gate dependencies that hold the node now, its own and
    /// those it inherits through its ancestors (requirements, conditions, openings), its
    /// children aside; one entry per dependency, its nearest holder's; empty when nothing
    /// holds it. `blocked_by` and `blocked_through` explain the structure; this answers
    /// "what does it wait on".
    #[serde(default)]
    pub waits_on: Explained<Blocker>,
    // PRD: F1.
    /// A pending auto-done milestone that reads as done.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub done_by_date: bool,
    // PRD: B6.
    /// A snooze that holds over the node: its own, else the one of the container it is
    /// held through (`snoozed_via`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snoozed: Option<SnoozeTarget>,
    // PRD: B6.
    /// The container whose snooze holds over the node, the reason a descendant names.
    /// Set even when the node's own snooze holds as well, so a node snoozed twice names its
    /// container and shows its own target in `snoozed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snoozed_via: Option<NodeKey>,
    // PRD: B10.
    /// A placeholder with no children that is not atomic.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub needs_breakdown: bool,
    /// Gravity and its contributors (Priority).
    pub gravity: Score,
    /// The contributors to gravity.
    pub gravity_from: Explained<Contribution>,
    /// A container's subtree gravity (Priority): what rides on its whole area, the counted
    /// weight of the container and its open descendants and everything downstream of any of
    /// them, each once. Never less than any member's `gravity`; none for a node with no
    /// children.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtree_gravity: Option<Score>,
    /// Unlocks (Priority).
    pub unlocks: Score,
    /// The nodes completing this would unblock.
    pub unlocks_from: Explained<Contribution>,
    /// Rank, for nodes in the normalization set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<Real>,
}

// PRD: C8.
/// A dependent that completing a node would not yet free, and what else it waits on (
/// Priority: Unlocks): one entry of node detail's `still_waiting`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HeldDependent {
    /// The direct dependent that stays held.
    pub node: NodeKey,
    /// What else it waits on, besides the node: its unsatisfied requirements, condition
    /// gates, and stage openings, each with how it arose. Never empty.
    pub also_waits_on: Vec<Blocker>,
}

// PRD: C8.
/// The dependents completing a node would not yet free (Priority: Unlocks): node
/// detail's `still_waiting`, its largest entries up to `explanation_entry_count_max` with the
/// total; the rest page through the explanations of `ExplainedField::StillWaiting`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StillWaiting {
    /// The dependents, largest weight first.
    pub entries: Vec<HeldDependent>,
    /// How many there are in all.
    pub total: u32,
}

impl StillWaiting {
    /// The list a response carries for the complete `held`: the first
    /// `explanation_entry_count_max` entries and the total.
    #[must_use]
    pub fn for_response(mut held: Vec<HeldDependent>) -> Self {
        let total = u32::try_from(held.len()).unwrap_or(u32::MAX);
        held.truncate(usize::try_from(EXPLANATION_ENTRY_COUNT_MAX).unwrap_or(usize::MAX));
        Self {
            entries: held,
            total,
        }
    }
}

// PRD: D5.
/// What a stalled journey waits on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum StallCause {
    /// A gating node.
    Gate(NodeKey),
    /// A snoozed node and its target.
    Snooze {
        /// The snoozed node.
        node: NodeKey,
        /// What it waits for.
        until: SnoozeTarget,
    },
    /// A node held off the acting frontier until a date: a milestone whose date is ahead
    /// (one that marks itself done then, or a pinned one), or work or a decision whose own
    /// not-before date is ahead.
    Scheduled {
        /// The node.
        node: NodeKey,
        /// The date it waits for.
        date: Date,
        /// It marks itself done on that date; otherwise someone finishes it once the date
        /// has come.
        #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
        marks_itself_done: bool,
    },
}

// PRD: D5.
/// The journey-level stalled diagnostic.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Stalled {
    /// What it waits on.
    pub waiting_on: Vec<StallCause>,
    /// Every in-scope non-terminal node is blocked: shown as "blocked".
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub all_blocked: bool,
}

// ARCHITECTURE: Read path: `Derived`.
/// A derived journey.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Derived {
    /// The today it was derived for.
    pub today: Date,
    /// Per node.
    pub nodes: BTreeMap<NodeKey, NodeDerived>,
    /// The frontier, ranked (Priority: ties by slack, gravity, key).
    pub frontier: Vec<NodeKey>,
    /// The acting frontier, ranked.
    pub acting_frontier: Vec<NodeKey>,
    /// The stalled diagnostic, when stalled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stalled: Option<Stalled>,
}

/// A node's newly stale reasons.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StaleConsequence {
    /// The node.
    pub node: NodeKey,
    /// The reasons it gained.
    pub reasons: BTreeSet<GuardFailure>,
}

// PRD: D4, D7.
/// Finished work newly undecided: a node completed or answered while
/// a decision its relevance reads is unanswered, so it may not apply. A warning, never a
/// rejection: the node stays undecided and is not forced into scope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UndecidedConsequence {
    /// The node.
    pub node: NodeKey,
    /// The open decisions its relevance waits on that it newly waits on.
    pub unanswered: BTreeSet<NodeKey>,
}

/// A new or larger shortfall.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShortfallConsequence {
    /// The node.
    pub node: NodeKey,
    /// The shortfall and its chain.
    pub shortfall: ShortChain,
}

// PRD: D7.
/// What an accepted patch or a proposal preview newly caused in derived state,
/// reported with the result and never stored.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Consequences {
    /// Nodes that became stale or gained a stale reason.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stale: Vec<StaleConsequence>,
    /// New or larger shortfalls.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shortfalls: Vec<ShortfallConsequence>,
    /// Newly overdue nodes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overdue: Vec<NodeKey>,
    /// Finished nodes newly undecided, which may not apply: a warning, never a rejection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub undecided: Vec<UndecidedConsequence>,
    /// The journey became stalled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stalled: Option<Stalled>,
    /// Informational, never a warning: nodes newly on the acting frontier, in key order. A
    /// node the write adds is not unlocked by it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unlocked: Vec<NodeKey>,
    /// Informational: nodes that were in scope and are now not relevant, in key order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub out_of_scope: Vec<NodeKey>,
    /// Informational: nodes that were not relevant and are now relevant or undecided, in key
    /// order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub into_scope: Vec<NodeKey>,
}

impl Consequences {
    /// The warning half (D7): whether anything went stale, short, overdue, undecided, or the
    /// journey stalled. The informational half (`unlocked`, `out_of_scope`, `into_scope`)
    /// never counts.
    #[must_use]
    pub fn has_warnings(&self) -> bool {
        !(self.stale.is_empty()
            && self.shortfalls.is_empty()
            && self.overdue.is_empty()
            && self.undecided.is_empty()
            && self.stalled.is_none())
    }
}

/// The rank constants (Priority: normative formulas, configurable constants, defaults
/// shown). The four coefficients sum to a finite number, so every rank is finite.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, try_from = "RankConstantsWire")]
pub struct RankConstants {
    /// Urgency's weight (0.40).
    pub urgency: Real,
    /// Lateness's weight (0.15).
    pub late: Real,
    /// Normalized gravity's weight (0.25).
    pub gravity: Real,
    /// Weight of normalized unlocks (0.20).
    pub unlocks: Real,
    /// Days of slack over which urgency rises from 0 to 1 (14).
    pub horizon_days: u32,
    /// An undecided node's weight factor in gravity and unlocks (0.5).
    pub undecided_discount: UndecidedDiscount,
    /// The factor a target owned by someone other than the node's owner gets in unlocks (2).
    pub other_owner_factor: OwnerFactor,
}

/// The rank constants as written, before their coefficients' sum is checked.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RankConstantsWire {
    urgency: Real,
    late: Real,
    gravity: Real,
    unlocks: Real,
    horizon_days: u32,
    undecided_discount: UndecidedDiscount,
    other_owner_factor: OwnerFactor,
}

impl TryFrom<RankConstantsWire> for RankConstants {
    type Error = String;

    fn try_from(wire: RankConstantsWire) -> Result<Self, String> {
        let sum = wire.urgency.get() + wire.late.get() + wire.gravity.get() + wire.unlocks.get();
        if !sum.is_finite() {
            return Err(format!(
                "the rank coefficients sum to {sum}: a rank must be a finite number"
            ));
        }
        Ok(Self {
            urgency: wire.urgency,
            late: wire.late,
            gravity: wire.gravity,
            unlocks: wire.unlocks,
            horizon_days: wire.horizon_days,
            undecided_discount: wire.undecided_discount,
            other_owner_factor: wire.other_owner_factor,
        })
    }
}

impl Default for RankConstants {
    /// The PRD's defaults (Priority).
    fn default() -> Self {
        Self {
            urgency: Real(0.40),
            late: Real(0.15),
            gravity: Real(0.25),
            unlocks: Real(0.20),
            horizon_days: 14,
            undecided_discount: Thousandths(500),
            other_owner_factor: Thousandths(2_000),
        }
    }
}

/// An IANA time zone name, the deployment's (A9). The engine never reads the zone database:
/// hosts compute today in it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimeZoneName(String);

impl TimeZoneName {
    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::str::FromStr for TimeZoneName {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '+');
        let within = Limit::TitleBytes.check(text.len()).is_ok();
        if text.is_empty() || !within || !text.chars().all(allowed) {
            return Err(format!("{text:?} is not a time zone name"));
        }
        Ok(Self(text.to_owned()))
    }
}

crate::serde_util::string_serde!(TimeZoneName, "TimeZoneName", "An IANA time zone name.");

// ARCHITECTURE: Terms: Derive inputs.
/// The inputs derive takes besides the journey,
/// supplied by the host and never read by the engine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeriveInputs {
    /// Today, in the deployment's time zone.
    pub today: Date,
    /// The deployment's time zone.
    pub timezone: TimeZoneName,
    /// The rank constants.
    pub rank: RankConstants,
    // PRD: H3.
    /// The viewer's identities, resolved from verified emails; usually one.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub viewer: BTreeSet<IdentityKey>,
    // PRD: E4, E6.
    /// The deployment context: identities, emails, and aliases at a revision.
    pub deployment: Deployment,
}

// ARCHITECTURE: Terms.
/// The domain document: a journey's graph and state, the inputs to
/// derive it. Nothing derived is in it; proposals are not part of it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DomainDocument {
    /// The journey.
    pub journey: Journey,
    /// The derive inputs.
    pub inputs: DeriveInputs,
}

impl fmt::Display for Relevance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Relevance::Relevant => "relevant",
            Relevance::NotRelevant => "not_relevant",
            Relevance::Undecided => "undecided",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_are_exact_millionths() {
        let score: Score = serde_json::from_str("2.5").unwrap();
        assert_eq!(score.millionths(), 2_500_000);
        assert_eq!(serde_json::to_string(&score).unwrap(), "2.5");
        let largest = Score::from_millionths(Score::MAX.millionths() - 1);
        let text = serde_json::to_string(&largest).unwrap();
        assert_eq!(serde_json::from_str::<Score>(&text).unwrap(), largest);
        for bad in ["-1", "0.0000001", "2.0000004", "-0.0000001", "1e20"] {
            assert!(serde_json::from_str::<Score>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn thousandths_are_exact_and_within_their_range() {
        let discount: UndecidedDiscount = serde_json::from_str("0.1").unwrap();
        assert_eq!(discount.get(), 100);
        assert_eq!(serde_json::to_string(&discount).unwrap(), "0.1");
        for bad in [
            "1.5",
            "-0.5",
            "0.0005",
            "-0.000001",
            "0.5000009",
            "1.0000009",
        ] {
            assert!(
                serde_json::from_str::<UndecidedDiscount>(bad).is_err(),
                "{bad}"
            );
        }
        assert!(
            serde_json::from_str::<OwnerFactor>("0.5").is_err(),
            "below one"
        );
        assert!(
            serde_json::from_str::<OwnerFactor>("1000").is_ok(),
            "weight_max"
        );
        assert!(serde_json::from_str::<OwnerFactor>("1000.001").is_err());
    }

    #[test]
    fn a_response_keeps_the_largest_explanations_and_the_total() {
        let entries: Vec<u32> = (0..60).collect();
        let complete = Explained::complete(entries).unwrap();
        let response = complete.for_response();
        assert_eq!(response.total, 60);
        assert_eq!(
            response.entries.len(),
            usize::try_from(Limit::ExplanationEntryCount.max()).unwrap()
        );
        assert_eq!(response.entries.as_slice().first(), Some(&0));
    }

    #[test]
    fn reals_are_finite_and_non_negative() {
        for bad in [".nan", ".inf", "-.inf", "-0.5"] {
            let yaml = format!(
                "urgency: {bad}\nlate: 0.15\ngravity: 0.25\nunlocks: 0.2\nhorizon_days: 14\nundecided_discount: 0.5\nother_owner_factor: 2\n"
            );
            assert!(crate::from_yaml::<RankConstants>(&yaml).is_err(), "{bad}");
        }
        assert!(Real::try_from(0.4).is_ok());
        let overflowing = "urgency: 1e308\nlate: 0.15\ngravity: 1e308\nunlocks: 0.2\nhorizon_days: 14\nundecided_discount: 0.5\nother_owner_factor: 2\n";
        assert!(
            crate::from_yaml::<RankConstants>(overflowing).is_err(),
            "coefficients summing past any finite rank"
        );
    }

    #[test]
    fn rank_constants_default_to_the_prd() {
        let constants = RankConstants::default();
        let sum = constants.urgency.get()
            + constants.late.get()
            + constants.gravity.get()
            + constants.unlocks.get();
        assert!((sum - 1.0).abs() < 1e-9);
    }
}
