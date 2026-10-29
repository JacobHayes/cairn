//! The node (A1a; ARCHITECTURE, Engine > Model): one struct of the fields every kind
//! shares and a kind-specific payload, so a field a kind cannot have is unrepresentable.
//! A node is written flat, as the PRD lists its fields:
//!
//! ```yaml
//! - id: who-owns
//!   kind: decision
//!   title: Evaluation owner
//!   prompt: Who owns the evaluation?
//!   answer_type: identity
//!   fills_role: eval_owner
//! ```
//!
//! and parsing it checks the kind restrictions, naming the field that does not belong.

use std::collections::BTreeMap;
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::attachment::Resource;
use crate::collections::{
    BoundedSet, BoundedVec, ChecklistItemCountPerNode, ChoiceCountPerDecision, CollectionError,
    EdgeCountPerNode, IdentityCountPerFill, OneOrMany, ResourceCountPerNode,
};
use crate::condition::Condition;
use crate::id::{IdentityKey, Slug};
use crate::limits::Limit;
use crate::number::{Days, Weight};
use crate::refs::{References, key_absent};
use crate::text::{Markdown, Title};

// PRD: glossary: a fixed set.
/// A node kind.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// A typed question.
    Decision,
    /// Work that produces an output.
    Deliverable,
    /// A step of work.
    Action,
    /// A point-in-time marker.
    Milestone,
    /// A pure container.
    Group,
}

impl NodeKind {
    /// Every kind.
    pub const ALL: [NodeKind; 5] = [
        NodeKind::Decision,
        NodeKind::Deliverable,
        NodeKind::Action,
        NodeKind::Milestone,
        NodeKind::Group,
    ];

    /// The kind as written.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            NodeKind::Decision => "decision",
            NodeKind::Deliverable => "deliverable",
            NodeKind::Action => "action",
            NodeKind::Milestone => "milestone",
            NodeKind::Group => "group",
        }
    }

    /// A9: every kind weighs 1 by default except a group, which weighs 0.
    #[must_use]
    pub const fn default_weight(self) -> Weight {
        match self {
            NodeKind::Group => Weight::GROUP_DEFAULT,
            NodeKind::Decision | NodeKind::Deliverable | NodeKind::Action | NodeKind::Milestone => {
                Weight::DEFAULT
            }
        }
    }

    /// A2: decisions and milestones are leaves; the other kinds may have children.
    #[must_use]
    pub const fn is_leaf(self) -> bool {
        match self {
            NodeKind::Decision | NodeKind::Milestone => true,
            NodeKind::Deliverable | NodeKind::Action | NodeKind::Group => false,
        }
    }
}

// PRD: A4.
/// A decision's answer type.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AnswerType {
    /// Yes or no.
    Boolean,
    /// One of the choices.
    SingleChoice,
    /// Any of the choices.
    MultiChoice,
    /// Free text.
    Text,
    /// A calendar date.
    Date,
    /// One identity.
    Identity,
    /// A list of identities.
    IdentityList,
}

impl AnswerType {
    /// Every answer type.
    pub const ALL: [AnswerType; 7] = [
        AnswerType::Boolean,
        AnswerType::SingleChoice,
        AnswerType::MultiChoice,
        AnswerType::Text,
        AnswerType::Date,
        AnswerType::Identity,
        AnswerType::IdentityList,
    ];

    /// Whether a condition can ask if an answer of this type equals a value (`equals`,
    /// `not_equals`, `in`).
    #[must_use]
    pub const fn compares_equal(self) -> bool {
        match self {
            AnswerType::Boolean
            | AnswerType::SingleChoice
            | AnswerType::Text
            | AnswerType::Date
            | AnswerType::Identity => true,
            AnswerType::MultiChoice | AnswerType::IdentityList => false,
        }
    }

    /// Whether a condition can ask if an answer of this type holds a value (`contains`).
    #[must_use]
    pub const fn compares_member(self) -> bool {
        match self {
            AnswerType::MultiChoice | AnswerType::IdentityList | AnswerType::Text => true,
            AnswerType::Boolean
            | AnswerType::SingleChoice
            | AnswerType::Date
            | AnswerType::Identity => false,
        }
    }

    /// A6: whether a decision of this type fills a multi-valued role (`Some(true)`), a
    /// single-valued one (`Some(false)`), or no role (`None`).
    #[must_use]
    pub const fn fills_role_multi(self) -> Option<bool> {
        match self {
            AnswerType::Identity => Some(false),
            AnswerType::IdentityList => Some(true),
            AnswerType::Boolean
            | AnswerType::SingleChoice
            | AnswerType::MultiChoice
            | AnswerType::Text
            | AnswerType::Date => None,
        }
    }

    /// The answer type as written.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            AnswerType::Boolean => "boolean",
            AnswerType::SingleChoice => "single_choice",
            AnswerType::MultiChoice => "multi_choice",
            AnswerType::Text => "text",
            AnswerType::Date => "date",
            AnswerType::Identity => "identity",
            AnswerType::IdentityList => "identity_list",
        }
    }
}

/// One choice of a single- or multi-choice decision: an id that answers and conditions use,
/// and an optional label. Written as the bare id when it has no label.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(try_from = "ChoiceWire", into = "ChoiceWire")]
#[schemars(with = "ChoiceWire")]
pub struct Choice {
    /// The id answers and conditions refer to.
    pub id: Slug,
    /// The label shown, if not the id.
    pub title: Option<Title>,
}

/// A choice as written: a bare id, or an id with a title.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
#[schemars(rename = "Choice")]
enum ChoiceWire {
    /// Just the id.
    Id(Slug),
    /// The id and a label.
    Labeled(LabeledChoice),
}

/// A choice with a label, as written. Unknown fields are an error, so a misspelled or
/// unsupported attribute is never silently dropped.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LabeledChoice {
    /// The id.
    pub id: Slug,
    /// The label.
    pub title: Title,
}

impl From<ChoiceWire> for Choice {
    fn from(wire: ChoiceWire) -> Self {
        match wire {
            ChoiceWire::Id(id) => Choice { id, title: None },
            ChoiceWire::Labeled(LabeledChoice { id, title }) => Choice {
                id,
                title: Some(title),
            },
        }
    }
}

impl From<Choice> for ChoiceWire {
    fn from(choice: Choice) -> Self {
        match choice.title {
            None => ChoiceWire::Id(choice.id),
            Some(title) => ChoiceWire::Labeled(LabeledChoice {
                id: choice.id,
                title,
            }),
        }
    }
}

/// A decision's choices: at least one, ids unique, at most `choice_count_per_decision_max`,
/// in the order written.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(
    try_from = "BoundedVec<Choice, ChoiceCountPerDecision>",
    into = "BoundedVec<Choice, ChoiceCountPerDecision>"
)]
pub struct Choices(BoundedVec<Choice, ChoiceCountPerDecision>);

impl JsonSchema for Choices {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Choices".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "array",
            "items": generator.subschema_for::<Choice>(),
            "minItems": 1,
            "maxItems": Limit::ChoiceCountPerDecision.max(),
        })
    }
}

impl Choices {
    /// The choices, in order.
    #[must_use]
    pub fn as_slice(&self) -> &[Choice] {
        self.0.as_slice()
    }
}

impl TryFrom<BoundedVec<Choice, ChoiceCountPerDecision>> for Choices {
    type Error = CollectionError;

    fn try_from(
        choices: BoundedVec<Choice, ChoiceCountPerDecision>,
    ) -> Result<Self, CollectionError> {
        if choices.is_empty() {
            return Err(CollectionError::Empty);
        }
        let ids: Vec<&Slug> = choices.as_slice().iter().map(|choice| &choice.id).collect();
        BoundedSet::<&Slug, ChoiceCountPerDecision>::new(ids)?;
        Ok(Self(choices))
    }
}

impl From<Choices> for BoundedVec<Choice, ChoiceCountPerDecision> {
    fn from(choices: Choices) -> Self {
        choices.0
    }
}

// PRD: G5.
/// One item of a checklist.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ChecklistItem {
    /// The id a tick names; unique within the checklist.
    pub id: Slug,
    /// What the item says.
    pub title: Title,
}

// PRD: G5.
/// A deliverable's or action's checklist: items in order, ids unique, at most
/// `checklist_item_count_per_node_max`. Empty is no checklist.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(
    try_from = "BoundedVec<ChecklistItem, ChecklistItemCountPerNode>",
    into = "BoundedVec<ChecklistItem, ChecklistItemCountPerNode>"
)]
pub struct Checklist(BoundedVec<ChecklistItem, ChecklistItemCountPerNode>);

impl JsonSchema for Checklist {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Checklist".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "array",
            "items": generator.subschema_for::<ChecklistItem>(),
            "maxItems": Limit::ChecklistItemCountPerNode.max(),
        })
    }
}

impl Checklist {
    /// The items, in order.
    #[must_use]
    pub fn items(&self) -> &[ChecklistItem] {
        self.0.as_slice()
    }

    /// The item ids, in order.
    pub fn ids(&self) -> impl Iterator<Item = &Slug> {
        self.items().iter().map(|item| &item.id)
    }

    /// Whether an item has the id.
    #[must_use]
    pub fn contains(&self, id: &Slug) -> bool {
        self.ids().any(|item| item == id)
    }

    /// The title of the item with the id.
    #[must_use]
    pub fn title(&self, id: &Slug) -> Option<&Title> {
        self.items()
            .iter()
            .find(|item| &item.id == id)
            .map(|item| &item.title)
    }

    /// Whether there are no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items().is_empty()
    }

    /// The number of items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items().len()
    }
}

impl TryFrom<BoundedVec<ChecklistItem, ChecklistItemCountPerNode>> for Checklist {
    type Error = CollectionError;

    fn try_from(
        items: BoundedVec<ChecklistItem, ChecklistItemCountPerNode>,
    ) -> Result<Self, CollectionError> {
        let ids: Vec<&Slug> = items.as_slice().iter().map(|item| &item.id).collect();
        BoundedSet::<&Slug, ChecklistItemCountPerNode>::new(ids)?;
        Ok(Self(items))
    }
}

impl From<Checklist> for BoundedVec<ChecklistItem, ChecklistItemCountPerNode> {
    fn from(checklist: Checklist) -> Self {
        checklist.0
    }
}

/// A decision's answer type with the fields only that type may carry (A1a, A6, E3).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AnswerSpec<R: References> {
    /// Yes or no.
    Boolean,
    /// One of the choices.
    SingleChoice(Choices),
    /// Any of the choices.
    MultiChoice(Choices),
    /// Free text.
    Text,
    /// A date, optionally pinning a milestone (`feeds_milestone`).
    Date {
        /// The milestone the answer pins.
        feeds_milestone: Option<R::Node>,
    },
    /// One identity, optionally filling a single-valued role.
    Identity {
        /// The role the answer fills.
        fills_role: Option<R::Role>,
    },
    /// A list of identities, optionally filling a multi-valued role.
    IdentityList {
        /// The role the answer fills.
        fills_role: Option<R::Role>,
    },
}

impl<R: References> AnswerSpec<R> {
    /// A4: whether `answer` is of this answer type and, for a choice, names only its choices.
    #[must_use]
    pub fn accepts(&self, answer: &crate::state::AnswerValue) -> bool {
        self.answer_type() == answer.answer_type()
            && match self {
                AnswerSpec::SingleChoice(choices) | AnswerSpec::MultiChoice(choices) => {
                    crate::proposal::removed_choices(answer, choices).is_empty()
                }
                AnswerSpec::Boolean
                | AnswerSpec::Text
                | AnswerSpec::Date { .. }
                | AnswerSpec::Identity { .. }
                | AnswerSpec::IdentityList { .. } => true,
            }
    }

    /// The answer type.
    #[must_use]
    pub fn answer_type(&self) -> AnswerType {
        match self {
            AnswerSpec::Boolean => AnswerType::Boolean,
            AnswerSpec::SingleChoice(_) => AnswerType::SingleChoice,
            AnswerSpec::MultiChoice(_) => AnswerType::MultiChoice,
            AnswerSpec::Text => AnswerType::Text,
            AnswerSpec::Date { .. } => AnswerType::Date,
            AnswerSpec::Identity { .. } => AnswerType::Identity,
            AnswerSpec::IdentityList { .. } => AnswerType::IdentityList,
        }
    }
}

/// A decision's own fields (A4).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Decision<R: References> {
    /// The question.
    pub prompt: Markdown,
    /// Help text.
    pub help: Option<Markdown>,
    /// The answer type and its fields.
    pub answer: AnswerSpec<R>,
}

/// A deliverable's own fields.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Deliverable {
    /// Days of its own work (A9).
    pub estimate: Option<Days>,
    /// Each journey must break it down before it can be completed (B10).
    pub placeholder: bool,
    /// An artifact link is required before `done` (G2).
    pub requires_artifact: bool,
    /// A journey note on the node itself is required before `done` (G4).
    pub requires_note: bool,
    /// Items to tick through (G5).
    pub checklist: Checklist,
    /// Every checklist item ticked is required before `done` (G5).
    pub requires_checklist: bool,
}

/// An action's own fields.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Action {
    /// Days of its own work (A9).
    pub estimate: Option<Days>,
    /// Each journey must break it down before it can be completed (B10).
    pub placeholder: bool,
    /// A journey note on the node itself is required before `done` (G4).
    pub requires_note: bool,
    /// Items to tick through (G5).
    pub checklist: Checklist,
    /// Every checklist item ticked is required before `done` (G5).
    pub requires_checklist: bool,
}

/// A milestone's own fields.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Milestone {
    /// The graph's final milestone (B11); at most one per graph.
    pub is_final: bool,
    /// Reads as done once its date arrives and its dependencies are satisfied (F1).
    pub auto_done: bool,
}

/// A group's own fields: its stage bounds (F4).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Group<R: References> {
    /// The milestone that opens the stage.
    pub opens_at: Option<R::Node>,
    /// The milestone that closes the stage.
    pub closes_at: Option<R::Node>,
    /// The stage requires its opening milestone (default true).
    pub gates: bool,
    /// The closing milestone bounds the group's finish (default true).
    pub closes: bool,
}

impl<R: References> Default for Group<R> {
    fn default() -> Self {
        Self {
            opens_at: None,
            closes_at: None,
            gates: flag_default(KindField::Gates),
            closes: flag_default(KindField::Closes),
        }
    }
}

/// The value of a flag field a node does not write (see [`KindField::default_flag`]).
fn flag_default(field: KindField) -> bool {
    let Some(flag) = field.default_flag() else {
        unreachable!("only flag fields have a default")
    };
    flag
}

/// The kind-specific part of a node.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Payload<R: References> {
    /// A decision.
    Decision(Decision<R>),
    /// A deliverable.
    Deliverable(Deliverable),
    /// An action.
    Action(Action),
    /// A milestone.
    Milestone(Milestone),
    /// A group.
    Group(Group<R>),
}

impl<R: References> Payload<R> {
    /// The kind this payload belongs to.
    #[must_use]
    pub fn kind(&self) -> NodeKind {
        match self {
            Payload::Decision(_) => NodeKind::Decision,
            Payload::Deliverable(_) => NodeKind::Deliverable,
            Payload::Action(_) => NodeKind::Action,
            Payload::Milestone(_) => NodeKind::Milestone,
            Payload::Group(_) => NodeKind::Group,
        }
    }

    /// The answer type, for a decision.
    #[must_use]
    pub fn answer_type(&self) -> Option<AnswerType> {
        match self {
            Payload::Decision(decision) => Some(decision.answer.answer_type()),
            Payload::Deliverable(_)
            | Payload::Action(_)
            | Payload::Milestone(_)
            | Payload::Group(_) => None,
        }
    }

    /// Days of its own work, for a deliverable or an action with an estimate.
    #[must_use]
    pub fn estimate(&self) -> Option<Days> {
        match self {
            Payload::Deliverable(Deliverable { estimate, .. })
            | Payload::Action(Action { estimate, .. }) => *estimate,
            Payload::Decision(_) | Payload::Milestone(_) | Payload::Group(_) => None,
        }
    }

    /// The checklist, for a deliverable or an action.
    #[must_use]
    pub fn checklist(&self) -> Option<&Checklist> {
        match self {
            Payload::Deliverable(Deliverable { checklist, .. })
            | Payload::Action(Action { checklist, .. }) => Some(checklist),
            Payload::Decision(_) | Payload::Milestone(_) | Payload::Group(_) => None,
        }
    }

    /// True for a deliverable or an action that needs every checklist item ticked before `done`.
    #[must_use]
    pub fn requires_checklist(&self) -> bool {
        match self {
            Payload::Deliverable(Deliverable {
                requires_checklist, ..
            })
            | Payload::Action(Action {
                requires_checklist, ..
            }) => *requires_checklist,
            Payload::Decision(_) | Payload::Milestone(_) | Payload::Group(_) => false,
        }
    }

    /// True for a deliverable or an action to be broken down in each journey.
    #[must_use]
    pub fn placeholder(&self) -> bool {
        match self {
            Payload::Deliverable(Deliverable { placeholder, .. })
            | Payload::Action(Action { placeholder, .. }) => *placeholder,
            Payload::Decision(_) | Payload::Milestone(_) | Payload::Group(_) => false,
        }
    }
}

/// Where a date rule measures from (A8): a milestone or a date decision's answer (both by
/// node reference; which one is the engine's check), or the journey's creation, written
/// `journey.created_at` (a path cannot contain a dot, so the two never collide).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DateSource<R: References> {
    /// The journey's `created_at`.
    CreatedAt,
    /// A milestone, or a date decision's answer.
    Node(R::Node),
}

const CREATED_AT: &str = "journey.created_at";

impl<R: References> fmt::Display for DateSource<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DateSource::CreatedAt => formatter.write_str(CREATED_AT),
            DateSource::Node(node) => node.fmt(formatter),
        }
    }
}

impl<R: References> Serialize for DateSource<R> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de, R: References> Deserialize<'de> for DateSource<R> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text == CREATED_AT {
            return Ok(DateSource::CreatedAt);
        }
        text.parse()
            .map(DateSource::Node)
            .map_err(serde::de::Error::custom)
    }
}

impl<R: References> JsonSchema for DateSource<R> {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        format!("DateSource{}", R::schema_name()).into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "anyOf": [{ "const": CREATED_AT }, generator.subschema_for::<R::Node>()],
        })
    }
}

/// Which way a date rule points.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// The node's instant falls at least `offset` days before each source.
    Before,
    /// The node's instant falls at least `offset` days after each source.
    After,
}

// PRD: A8.
/// A date rule: `{before|after: <source(s)>, offset: <days>}`. Several sources are
/// several constraints; the offset defaults to 0.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(try_from = "DateRuleWire<R>", into = "DateRuleWire<R>", bound = "")]
#[schemars(with = "DateRuleWire<R>", bound = "R: References")]
pub struct DateRule<R: References> {
    /// Before or after the sources.
    pub direction: Direction,
    /// The sources, at most `edge_count_per_node_max` (each is one constraint).
    pub sources: OneOrMany<DateSource<R>, EdgeCountPerNode>,
    /// Days between, at most `offset_days_max`.
    pub offset: Days,
}

/// A date rule as written.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, bound = "")]
#[schemars(bound = "R: References", rename = "DateRule{R}", transform = date_rule_schema)]
struct DateRuleWire<R: References> {
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "OneOrMany<DateSource<R>, EdgeCountPerNode>")]
    before: Option<OneOrMany<DateSource<R>, EdgeCountPerNode>>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "OneOrMany<DateSource<R>, EdgeCountPerNode>")]
    after: Option<OneOrMany<DateSource<R>, EdgeCountPerNode>>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Days")]
    offset: Option<Days>,
}

fn date_rule_schema(schema: &mut schemars::Schema) {
    crate::serde_util::exactly_one_of(schema, &["before", "after"]);
}

/// A date rule with both or neither of `before` and `after`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DateRuleError;

impl fmt::Display for DateRuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a date rule has exactly one of `before` and `after`")
    }
}

impl std::error::Error for DateRuleError {}

impl<R: References> TryFrom<DateRuleWire<R>> for DateRule<R> {
    type Error = DateRuleError;

    fn try_from(wire: DateRuleWire<R>) -> Result<Self, DateRuleError> {
        let (direction, sources) = match (wire.before, wire.after) {
            (Some(sources), None) => (Direction::Before, sources),
            (None, Some(sources)) => (Direction::After, sources),
            (Some(_), Some(_)) | (None, None) => return Err(DateRuleError),
        };
        let zero = Days::try_from(0).map_err(|_| DateRuleError)?;
        Ok(Self {
            direction,
            sources,
            offset: wire.offset.unwrap_or(zero),
        })
    }
}

impl<R: References> From<DateRule<R>> for DateRuleWire<R> {
    fn from(rule: DateRule<R>) -> Self {
        let offset = (rule.offset.get() != 0).then_some(rule.offset);
        match rule.direction {
            Direction::Before => DateRuleWire {
                before: Some(rule.sources),
                after: None,
                offset,
            },
            Direction::After => DateRuleWire {
                before: None,
                after: Some(rule.sources),
                offset,
            },
        }
    }
}

/// Explicit identities on a node: a set, possibly empty (E2: explicit empty means none and
/// stops inheritance), at most `identity_count_per_fill_max`.
pub type IdentitySet = BoundedSet<IdentityKey, IdentityCountPerFill>;

// PRD: E2.
/// Where a participation comes from: a role reference, written as the role, or
/// explicit identities, written as a list.
//
// Read by hand rather than as an untagged enum, whose only error is "did not match any
// variant": a list past `identity_count_per_fill_max` is rejected naming that limit, a repeated
// identity naming it, and anything but a string or a list as such.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, JsonSchema)]
#[serde(untagged, bound = "")]
#[schemars(bound = "R: References", rename = "ParticipationSource{R}")]
pub enum ParticipationSource<R: References> {
    /// Whoever fills the role.
    Role(R::Role),
    /// These identities; an empty list is an explicit "none".
    Identities(IdentitySet),
}

impl<'de, R: References> Deserialize<'de> for ParticipationSource<R> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Source<R>(std::marker::PhantomData<R>);

        impl<'de, R: References> serde::de::Visitor<'de> for Source<R> {
            type Value = ParticipationSource<R>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a role, or a list of identities")
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Self::Value, E> {
                text.parse()
                    .map(ParticipationSource::Role)
                    .map_err(E::custom)
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut access: A,
            ) -> Result<Self::Value, A::Error> {
                let mut identities = Vec::new();
                while let Some(identity) = access.next_element::<IdentityKey>()? {
                    identities.push(identity);
                    // Checked as entries arrive, so an oversized list fails before it is read.
                    Limit::IdentityCountPerFill
                        .check(identities.len())
                        .map_err(serde::de::Error::custom)?;
                }
                IdentitySet::new(identities)
                    .map(ParticipationSource::Identities)
                    .map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_any(Source(std::marker::PhantomData))
    }
}

/// A node's participations (E2): at most one source per kind, at most `kind_count_max`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(into = "BTreeMap<R::Kind, ParticipationSource<R>>", bound = "")]
pub struct Participations<R: References>(BTreeMap<R::Kind, ParticipationSource<R>>);

impl<'de, R: References> Deserialize<'de> for Participations<R> {
    /// Reads the map entry by entry, so a kind written twice is an error rather than the
    /// last assignment silently winning, and the kind count is checked as entries arrive.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        crate::serde_util::unique_entries(
            deserializer,
            "a map from participation kind to a role or a list of identities",
            Some(Limit::KindCount),
        )
        .map(Participations)
    }
}

impl<R: References> JsonSchema for Participations<R> {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        format!("Participations{}", R::schema_name()).into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "object",
            "additionalProperties": generator.subschema_for::<ParticipationSource<R>>(),
            "maxProperties": Limit::KindCount.max(),
        })
    }
}

impl<R: References> Participations<R> {
    /// The sources by kind.
    #[must_use]
    pub fn as_map(&self) -> &BTreeMap<R::Kind, ParticipationSource<R>> {
        &self.0
    }

    /// True when the node declares none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<R: References> Default for Participations<R> {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}

impl<R: References> TryFrom<BTreeMap<R::Kind, ParticipationSource<R>>> for Participations<R> {
    type Error = crate::limits::LimitExceeded;

    fn try_from(map: BTreeMap<R::Kind, ParticipationSource<R>>) -> Result<Self, Self::Error> {
        Limit::KindCount.check(map.len())?;
        Ok(Self(map))
    }
}

impl<R: References> From<Participations<R>> for BTreeMap<R::Kind, ParticipationSource<R>> {
    fn from(participations: Participations<R>) -> Self {
        participations.0
    }
}

/// A node's explicit `requires` edges, at most `edge_count_per_node_max` (the in-plus-out
/// total is a graph-level check).
pub type Requires<R> = BoundedSet<<R as References>::Node, EdgeCountPerNode>;

// PRD: A1a.
/// A node.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(try_from = "NodeWire<R>", into = "NodeWire<R>", bound = "")]
#[schemars(with = "NodeWire<R>", bound = "R: References")]
pub struct Node<R: References> {
    /// The stable key: optional in a file, minted on import when absent.
    pub key: R::NodeKey,
    /// The id, unique among siblings.
    pub id: Slug,
    /// The parent, or none for a root.
    pub parent: Option<R::Node>,
    /// The title.
    pub title: Title,
    /// A markdown description.
    pub description: Option<Markdown>,
    /// The authored weight, when not the kind's default (A9).
    pub weight: Option<Weight>,
    /// Explicit `requires` edges: the nodes this one requires (A3).
    pub requires: Requires<R>,
    /// When the node is relevant (A5).
    pub relevant_when: Option<Condition<R>>,
    /// The latest finish (A8).
    pub due_by: Option<DateRule<R>>,
    /// The earliest start (A8).
    pub not_before: Option<DateRule<R>>,
    /// Default participations (E2).
    pub participations: Participations<R>,
    /// Route-authored guidance (A10), at most `resource_count_per_node_max`. Held as a list
    /// the engine edits in place, so validation checks the count after every patch as the
    /// parse checks it on read.
    pub resources: Vec<Resource<R>>,
    /// The kind and its own fields.
    pub payload: Payload<R>,
}

impl<R: References> Node<R> {
    /// The kind.
    #[must_use]
    pub fn kind(&self) -> NodeKind {
        self.payload.kind()
    }

    /// The weight that counts: the authored one, or the kind's default (A9).
    #[must_use]
    pub fn effective_weight(&self) -> Weight {
        self.weight.unwrap_or_else(|| self.kind().default_weight())
    }
}

/// A field of a node as written, for the kind restrictions (A1a) and their errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KindField {
    /// `estimate`: deliverables and actions.
    Estimate,
    /// `placeholder`: deliverables and actions.
    Placeholder,
    /// `requires_artifact`: deliverables.
    RequiresArtifact,
    /// `requires_note`: deliverables and actions.
    RequiresNote,
    /// `checklist`: deliverables and actions.
    Checklist,
    /// `requires_checklist`: deliverables and actions.
    RequiresChecklist,
    /// `final`: milestones.
    Final,
    /// `auto_done`: milestones.
    AutoDone,
    /// `opens_at`: groups.
    OpensAt,
    /// `closes_at`: groups.
    ClosesAt,
    /// `gates`: groups.
    Gates,
    /// `closes`: groups.
    Closes,
    /// `prompt`: decisions (required).
    Prompt,
    /// `help`: decisions.
    Help,
    /// `answer_type`: decisions (required).
    AnswerType,
    /// `choices`: single- and multi-choice decisions (required there).
    Choices,
    /// `fills_role`: identity and identity-list decisions.
    FillsRole,
    /// `feeds_milestone`: date decisions.
    FeedsMilestone,
}

impl KindField {
    /// Every kind-restricted field.
    pub const ALL: [KindField; 18] = [
        KindField::Estimate,
        KindField::RequiresArtifact,
        KindField::RequiresNote,
        KindField::Checklist,
        KindField::RequiresChecklist,
        KindField::Placeholder,
        KindField::Final,
        KindField::AutoDone,
        KindField::OpensAt,
        KindField::ClosesAt,
        KindField::Gates,
        KindField::Closes,
        KindField::Prompt,
        KindField::Help,
        KindField::AnswerType,
        KindField::Choices,
        KindField::FillsRole,
        KindField::FeedsMilestone,
    ];

    /// The field's name as written.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            KindField::Estimate => "estimate",
            KindField::Placeholder => "placeholder",
            KindField::RequiresArtifact => "requires_artifact",
            KindField::RequiresNote => "requires_note",
            KindField::Checklist => "checklist",
            KindField::RequiresChecklist => "requires_checklist",
            KindField::Final => "final",
            KindField::AutoDone => "auto_done",
            KindField::OpensAt => "opens_at",
            KindField::ClosesAt => "closes_at",
            KindField::Gates => "gates",
            KindField::Closes => "closes",
            KindField::Prompt => "prompt",
            KindField::Help => "help",
            KindField::AnswerType => "answer_type",
            KindField::Choices => "choices",
            KindField::FillsRole => "fills_role",
            KindField::FeedsMilestone => "feeds_milestone",
        }
    }

    /// The value a flag field has when a node does not write it: `gates` and `closes` are on,
    /// every other flag off. `None` for a field that is not a flag.
    #[must_use]
    pub const fn default_flag(self) -> Option<bool> {
        match self {
            KindField::Gates | KindField::Closes => Some(true),
            KindField::Placeholder
            | KindField::RequiresArtifact
            | KindField::RequiresNote
            | KindField::RequiresChecklist
            | KindField::Final
            | KindField::AutoDone => Some(false),
            KindField::Estimate
            | KindField::OpensAt
            | KindField::ClosesAt
            | KindField::Prompt
            | KindField::Help
            | KindField::AnswerType
            | KindField::Checklist
            | KindField::Choices
            | KindField::FillsRole
            | KindField::FeedsMilestone => None,
        }
    }

    /// A1a: whether a node of `kind` may carry the field. For the decision fields that
    /// depend on the answer type, see [`KindField::allowed_for_answer`].
    #[must_use]
    pub const fn allowed_on(self, kind: NodeKind) -> bool {
        match self {
            KindField::Estimate
            | KindField::Placeholder
            | KindField::RequiresNote
            | KindField::Checklist
            | KindField::RequiresChecklist => {
                matches!(kind, NodeKind::Deliverable | NodeKind::Action)
            }
            KindField::RequiresArtifact => matches!(kind, NodeKind::Deliverable),
            KindField::Final | KindField::AutoDone => matches!(kind, NodeKind::Milestone),
            KindField::OpensAt | KindField::ClosesAt | KindField::Gates | KindField::Closes => {
                matches!(kind, NodeKind::Group)
            }
            KindField::Prompt
            | KindField::Help
            | KindField::AnswerType
            | KindField::Choices
            | KindField::FillsRole
            | KindField::FeedsMilestone => matches!(kind, NodeKind::Decision),
        }
    }

    /// A1a: whether a node of `kind`, and for a decision its answer type, may carry the
    /// field: the one table every read and write of a kind field asks.
    #[must_use]
    pub const fn allowed(self, kind: NodeKind, answer: Option<AnswerType>) -> bool {
        self.allowed_on(kind)
            && match answer {
                Some(answer) => self.allowed_for_answer(answer),
                None => true,
            }
    }

    /// A1a: whether a decision with `answer` may carry the field (true for every field not
    /// tied to an answer type).
    #[must_use]
    pub const fn allowed_for_answer(self, answer: AnswerType) -> bool {
        match self {
            KindField::Choices => {
                matches!(answer, AnswerType::SingleChoice | AnswerType::MultiChoice)
            }
            KindField::FillsRole => {
                matches!(answer, AnswerType::Identity | AnswerType::IdentityList)
            }
            KindField::FeedsMilestone => matches!(answer, AnswerType::Date),
            KindField::Estimate
            | KindField::Placeholder
            | KindField::RequiresArtifact
            | KindField::RequiresNote
            | KindField::Checklist
            | KindField::RequiresChecklist
            | KindField::Final
            | KindField::AutoDone
            | KindField::OpensAt
            | KindField::ClosesAt
            | KindField::Gates
            | KindField::Closes
            | KindField::Prompt
            | KindField::Help
            | KindField::AnswerType => true,
        }
    }
}

/// A node whose fields do not fit its kind (A1a).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeShapeError {
    /// A field the node's kind (or answer type) cannot have.
    NotAllowed {
        /// The field.
        field: KindField,
        /// The node's kind.
        kind: NodeKind,
        /// The decision's answer type, when the restriction is by answer type.
        answer: Option<AnswerType>,
    },
    /// A field the node's kind (or answer type) requires.
    Missing {
        /// The field.
        field: KindField,
        /// The node's kind.
        kind: NodeKind,
    },
}

impl fmt::Display for NodeShapeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeShapeError::NotAllowed {
                field,
                kind,
                answer: None,
            } => write!(
                formatter,
                "field `{}` is not allowed on a {} node",
                field.name(),
                kind.name()
            ),
            NodeShapeError::NotAllowed {
                field,
                kind,
                answer: Some(answer),
            } => write!(
                formatter,
                "field `{}` is not allowed on a {} node with answer_type {}",
                field.name(),
                kind.name(),
                answer.name()
            ),
            NodeShapeError::Missing { field, kind } => {
                write!(
                    formatter,
                    "a {} node requires field `{}`",
                    kind.name(),
                    field.name()
                )
            }
        }
    }
}

impl std::error::Error for NodeShapeError {}

/// A node as written: every field any kind may carry, flat. Converting it into a [`Node`]
/// checks the kind restrictions; converting back writes only the fields the kind has. A
/// kind-restricted field may not be written `null`, so absence is the only way to leave it
/// out and the parser and the JSON Schema agree.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, bound = "")]
#[schemars(bound = "R: References", rename = "Node{R}", transform = crate::node_schema::restrict_by_kind)]
struct NodeWire<R: References> {
    #[serde(skip_serializing_if = "key_absent")]
    key: R::NodeKey,
    id: Slug,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<R::Node>,
    kind: NodeKind,
    title: Title,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<Markdown>,
    #[serde(skip_serializing_if = "Option::is_none")]
    weight: Option<Weight>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Markdown")]
    prompt: Option<Markdown>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "AnswerType")]
    answer_type: Option<AnswerType>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Choices")]
    choices: Option<Choices>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "R::Role")]
    fills_role: Option<R::Role>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "R::Node")]
    feeds_milestone: Option<R::Node>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Markdown")]
    help: Option<Markdown>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Days")]
    estimate: Option<Days>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    placeholder: Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    requires_artifact: Option<bool>,
    /// Completing needs a note on the node itself, not a child's or the journey's.
    #[schemars(extend("x-sketch" = true))]
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    requires_note: Option<bool>,
    /// Setting it replaces the whole list.
    #[schemars(extend("x-sketch" = true))]
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Checklist")]
    checklist: Option<Checklist>,
    /// Completing needs every checklist item ticked.
    #[schemars(extend("x-sketch" = true))]
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    requires_checklist: Option<bool>,
    #[serde(
        rename = "final",
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    is_final: Option<bool>,
    /// A milestone reads `scheduled` until its date, then ready to mark done; with this it is done
    /// by itself. Marking it done early records when.
    #[schemars(extend("x-sketch" = true))]
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    auto_done: Option<bool>,
    /// The milestone that opens the stage: the contents wait for it unless `gates` is false.
    #[schemars(extend("x-sketch" = true))]
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "R::Node")]
    opens_at: Option<R::Node>,
    /// The milestone that closes the stage: it bounds the group's finish and its contents' due
    /// dates unless `closes` is false. Consecutive stages share one.
    #[schemars(extend("x-sketch" = true))]
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "R::Node")]
    closes_at: Option<R::Node>,
    /// False lets the contents be done before the opening milestone. A stage holds the work done
    /// during it; work that can as well be done ahead goes in a stage with this false; prep that
    /// must come first sits outside, due before the opening milestone.
    #[schemars(extend("x-sketch" = true))]
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    gates: Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "bool")]
    closes: Option<bool>,
    #[serde(default, skip_serializing_if = "BoundedSet::is_empty")]
    requires: Requires<R>,
    #[serde(skip_serializing_if = "Option::is_none")]
    relevant_when: Option<Condition<R>>,
    /// The latest finish, a node or the journey's creation (a fixed date is a pinned milestone);
    /// a deadline only, it never holds work back.
    #[schemars(extend("x-sketch" = true))]
    #[serde(skip_serializing_if = "Option::is_none")]
    due_by: Option<DateRule<R>>,
    /// The earliest start: until then the node reads `scheduled` and stays off the next list, yet
    /// can be started early. What waits on it is `blocked`, its `waits_on` carrying the date.
    #[schemars(extend("x-sketch" = true))]
    #[serde(skip_serializing_if = "Option::is_none")]
    not_before: Option<DateRule<R>>,
    #[serde(default, skip_serializing_if = "Participations::is_empty")]
    participations: Participations<R>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "node_resources"
    )]
    #[schemars(with = "BoundedVec<Resource<R>, ResourceCountPerNode>")]
    resources: Vec<Resource<R>>,
}

/// A node's resources as written: at most `resource_count_per_node_max`.
fn node_resources<'de, D: serde::Deserializer<'de>, R: References>(
    deserializer: D,
) -> Result<Vec<Resource<R>>, D::Error> {
    BoundedVec::<Resource<R>, ResourceCountPerNode>::deserialize(deserializer)
        .map(BoundedVec::into_vec)
}

// PRD: A1a.
/// A node's kind-specific fields as flat optional columns: the file and wire form's fields
/// and the store's columns. Each is empty unless the node's kind (and for a decision, its
/// answer type) has it. The one conversion between a payload and its flat fields, made once
/// per node by the wire form and the store; it clones a decision's text, so it is not an
/// access path for a single field.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::struct_field_names, missing_docs)]
pub struct KindColumns<R: References> {
    pub estimate: Option<Days>,
    pub placeholder: Option<bool>,
    pub requires_artifact: Option<bool>,
    pub requires_note: Option<bool>,
    pub checklist: Option<Checklist>,
    pub requires_checklist: Option<bool>,
    pub is_final: Option<bool>,
    pub auto_done: Option<bool>,
    pub opens_at: Option<R::Node>,
    pub closes_at: Option<R::Node>,
    pub gates: Option<bool>,
    pub closes: Option<bool>,
    pub prompt: Option<Markdown>,
    pub answer_type: Option<AnswerType>,
    pub choices: Option<Choices>,
    pub fills_role: Option<R::Role>,
    pub feeds_milestone: Option<R::Node>,
    pub help: Option<Markdown>,
}

impl<R: References> Default for KindColumns<R> {
    fn default() -> Self {
        Self {
            estimate: None,
            placeholder: None,
            requires_artifact: None,
            requires_note: None,
            checklist: None,
            requires_checklist: None,
            is_final: None,
            auto_done: None,
            opens_at: None,
            closes_at: None,
            gates: None,
            closes: None,
            prompt: None,
            answer_type: None,
            choices: None,
            fills_role: None,
            feeds_milestone: None,
            help: None,
        }
    }
}

impl<R: References> KindColumns<R> {
    /// The columns a decision fills: its question, help, and the fields of its answer type.
    fn of_decision(decision: &Decision<R>) -> Self {
        let Decision {
            prompt,
            help,
            answer,
        } = decision;
        let mut columns = Self {
            prompt: Some(prompt.clone()),
            help: help.clone(),
            answer_type: Some(answer.answer_type()),
            ..Self::default()
        };
        match answer {
            AnswerSpec::Boolean | AnswerSpec::Text => {}
            AnswerSpec::SingleChoice(choices) | AnswerSpec::MultiChoice(choices) => {
                columns.choices = Some(choices.clone());
            }
            AnswerSpec::Date { feeds_milestone } => {
                columns.feeds_milestone.clone_from(feeds_milestone);
            }
            AnswerSpec::Identity { fills_role } | AnswerSpec::IdentityList { fills_role } => {
                columns.fills_role.clone_from(fills_role);
            }
        }
        columns
    }

    /// The columns a payload fills: every field its kind has, flags written explicitly.
    #[must_use]
    pub fn of(payload: &Payload<R>) -> Self {
        match payload {
            Payload::Decision(decision) => Self::of_decision(decision),
            Payload::Deliverable(Deliverable {
                estimate,
                placeholder,
                requires_artifact,
                requires_note,
                checklist,
                requires_checklist,
            }) => Self {
                estimate: *estimate,
                placeholder: Some(*placeholder),
                requires_artifact: Some(*requires_artifact),
                requires_note: Some(*requires_note),
                checklist: Some(checklist.clone()),
                requires_checklist: Some(*requires_checklist),
                ..Self::default()
            },
            Payload::Action(Action {
                estimate,
                placeholder,
                requires_note,
                checklist,
                requires_checklist,
            }) => Self {
                estimate: *estimate,
                placeholder: Some(*placeholder),
                requires_note: Some(*requires_note),
                checklist: Some(checklist.clone()),
                requires_checklist: Some(*requires_checklist),
                ..Self::default()
            },
            Payload::Milestone(Milestone {
                is_final,
                auto_done,
            }) => Self {
                is_final: Some(*is_final),
                auto_done: Some(*auto_done),
                ..Self::default()
            },
            Payload::Group(Group {
                opens_at,
                closes_at,
                gates,
                closes,
            }) => Self {
                opens_at: opens_at.clone(),
                closes_at: closes_at.clone(),
                gates: Some(*gates),
                closes: Some(*closes),
                ..Self::default()
            },
        }
    }

    /// The columns with every flag at its default left out, so a canonical file has one
    /// spelling per node.
    #[must_use]
    pub fn without_defaults(mut self) -> Self {
        let set = |flag: Option<bool>, default: bool| flag.filter(|flag| *flag != default);
        self.placeholder = set(self.placeholder, false);
        self.requires_artifact = set(self.requires_artifact, false);
        self.requires_note = set(self.requires_note, false);
        self.requires_checklist = set(self.requires_checklist, false);
        self.checklist = self.checklist.filter(|checklist| !checklist.is_empty());
        self.is_final = set(self.is_final, false);
        self.auto_done = set(self.auto_done, false);
        self.gates = set(self.gates, true);
        self.closes = set(self.closes, true);
        self
    }

    /// The kind-restricted fields with a value.
    #[must_use]
    pub fn present(&self) -> Vec<KindField> {
        let Self {
            estimate,
            placeholder,
            requires_artifact,
            requires_note,
            checklist,
            requires_checklist,
            is_final,
            auto_done,
            opens_at,
            closes_at,
            gates,
            closes,
            prompt,
            answer_type,
            choices,
            fills_role,
            feeds_milestone,
            help,
        } = self;
        [
            (KindField::Estimate, estimate.is_some()),
            (KindField::RequiresArtifact, requires_artifact.is_some()),
            (KindField::RequiresNote, requires_note.is_some()),
            (KindField::Checklist, checklist.is_some()),
            (KindField::RequiresChecklist, requires_checklist.is_some()),
            (KindField::Placeholder, placeholder.is_some()),
            (KindField::Final, is_final.is_some()),
            (KindField::AutoDone, auto_done.is_some()),
            (KindField::OpensAt, opens_at.is_some()),
            (KindField::ClosesAt, closes_at.is_some()),
            (KindField::Gates, gates.is_some()),
            (KindField::Closes, closes.is_some()),
            (KindField::Prompt, prompt.is_some()),
            (KindField::Help, help.is_some()),
            (KindField::AnswerType, answer_type.is_some()),
            (KindField::Choices, choices.is_some()),
            (KindField::FillsRole, fills_role.is_some()),
            (KindField::FeedsMilestone, feeds_milestone.is_some()),
        ]
        .into_iter()
        .filter_map(|(field, present)| present.then_some(field))
        .collect()
    }

    /// A decision's own fields: its prompt and answer type are required, and the answer type
    /// requires its choices.
    fn into_decision(self) -> Result<Decision<R>, NodeShapeError> {
        let missing = |field| NodeShapeError::Missing {
            field,
            kind: NodeKind::Decision,
        };
        let prompt = self.prompt.ok_or_else(|| missing(KindField::Prompt))?;
        let answer = self
            .answer_type
            .ok_or_else(|| missing(KindField::AnswerType))?;
        let choices = self.choices.ok_or_else(|| missing(KindField::Choices));
        let answer = match answer {
            AnswerType::Boolean => AnswerSpec::Boolean,
            AnswerType::SingleChoice => AnswerSpec::SingleChoice(choices?),
            AnswerType::MultiChoice => AnswerSpec::MultiChoice(choices?),
            AnswerType::Text => AnswerSpec::Text,
            AnswerType::Date => AnswerSpec::Date {
                feeds_milestone: self.feeds_milestone,
            },
            AnswerType::Identity => AnswerSpec::Identity {
                fills_role: self.fills_role,
            },
            AnswerType::IdentityList => AnswerSpec::IdentityList {
                fills_role: self.fills_role,
            },
        };
        Ok(Decision {
            prompt,
            help: self.help,
            answer,
        })
    }

    /// The payload of a node of `kind`. A missing flag is its default.
    ///
    /// # Errors
    ///
    /// A field the kind or answer type cannot have, or a missing required one (A1a).
    pub fn into_payload(self, kind: NodeKind) -> Result<Payload<R>, NodeShapeError> {
        for field in self.present() {
            if !field.allowed_on(kind) {
                return Err(NodeShapeError::NotAllowed {
                    field,
                    kind,
                    answer: None,
                });
            }
            if let Some(answer) = self.answer_type
                && !field.allowed_for_answer(answer)
            {
                return Err(NodeShapeError::NotAllowed {
                    field,
                    kind,
                    answer: Some(answer),
                });
            }
        }
        Ok(match kind {
            NodeKind::Decision => Payload::Decision(self.into_decision()?),
            NodeKind::Deliverable => Payload::Deliverable(Deliverable {
                estimate: self.estimate,
                placeholder: self
                    .placeholder
                    .unwrap_or(flag_default(KindField::Placeholder)),
                requires_artifact: self
                    .requires_artifact
                    .unwrap_or(flag_default(KindField::RequiresArtifact)),
                requires_note: self
                    .requires_note
                    .unwrap_or(flag_default(KindField::RequiresNote)),
                checklist: self.checklist.unwrap_or_default(),
                requires_checklist: self
                    .requires_checklist
                    .unwrap_or(flag_default(KindField::RequiresChecklist)),
            }),
            NodeKind::Action => Payload::Action(Action {
                estimate: self.estimate,
                placeholder: self
                    .placeholder
                    .unwrap_or(flag_default(KindField::Placeholder)),
                requires_note: self
                    .requires_note
                    .unwrap_or(flag_default(KindField::RequiresNote)),
                checklist: self.checklist.unwrap_or_default(),
                requires_checklist: self
                    .requires_checklist
                    .unwrap_or(flag_default(KindField::RequiresChecklist)),
            }),
            NodeKind::Milestone => Payload::Milestone(Milestone {
                is_final: self.is_final.unwrap_or(flag_default(KindField::Final)),
                auto_done: self.auto_done.unwrap_or(flag_default(KindField::AutoDone)),
            }),
            NodeKind::Group => Payload::Group(Group {
                opens_at: self.opens_at,
                closes_at: self.closes_at,
                gates: self.gates.unwrap_or(flag_default(KindField::Gates)),
                closes: self.closes.unwrap_or(flag_default(KindField::Closes)),
            }),
        })
    }
}

impl<R: References> TryFrom<NodeWire<R>> for Node<R> {
    type Error = NodeShapeError;

    fn try_from(wire: NodeWire<R>) -> Result<Self, NodeShapeError> {
        let payload = KindColumns {
            estimate: wire.estimate,
            placeholder: wire.placeholder,
            requires_artifact: wire.requires_artifact,
            requires_note: wire.requires_note,
            checklist: wire.checklist,
            requires_checklist: wire.requires_checklist,
            is_final: wire.is_final,
            auto_done: wire.auto_done,
            opens_at: wire.opens_at,
            closes_at: wire.closes_at,
            gates: wire.gates,
            closes: wire.closes,
            prompt: wire.prompt,
            answer_type: wire.answer_type,
            choices: wire.choices,
            fills_role: wire.fills_role,
            feeds_milestone: wire.feeds_milestone,
            help: wire.help,
        }
        .into_payload(wire.kind)?;
        Ok(Self {
            key: wire.key,
            id: wire.id,
            parent: wire.parent,
            title: wire.title,
            description: wire.description,
            weight: wire.weight,
            requires: wire.requires,
            relevant_when: wire.relevant_when,
            due_by: wire.due_by,
            not_before: wire.not_before,
            participations: wire.participations,
            resources: wire.resources,
            payload,
        })
    }
}

impl<R: References> From<Node<R>> for NodeWire<R> {
    fn from(node: Node<R>) -> Self {
        let KindColumns {
            estimate,
            placeholder,
            requires_artifact,
            requires_note,
            checklist,
            requires_checklist,
            is_final,
            auto_done,
            opens_at,
            closes_at,
            gates,
            closes,
            prompt,
            answer_type,
            choices,
            fills_role,
            feeds_milestone,
            help,
        } = KindColumns::of(&node.payload).without_defaults();
        NodeWire {
            key: node.key,
            id: node.id,
            parent: node.parent,
            kind: node.payload.kind(),
            title: node.title,
            description: node.description,
            weight: node.weight,
            prompt,
            answer_type,
            choices,
            fills_role,
            feeds_milestone,
            help,
            estimate,
            placeholder,
            requires_artifact,
            requires_note,
            checklist,
            requires_checklist,
            is_final,
            auto_done,
            opens_at,
            closes_at,
            gates,
            closes,
            requires: node.requires,
            relevant_when: node.relevant_when,
            due_by: node.due_by,
            not_before: node.not_before,
            participations: node.participations,
            resources: node.resources,
        }
    }
}

impl crate::collections::HasKey for Node<crate::refs::KeyRefs> {
    type Key = crate::id::NodeKey;

    fn key(&self) -> &crate::id::NodeKey {
        &self.key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refs::FileRefs;
    use serde_json::{Value, json};

    fn decision(answer_type: &str, extra: &Value) -> Value {
        let mut node = json!({"id": "probe", "kind": "decision", "title": "Probe", "prompt": "Which?", "answer_type": answer_type});
        for (field, value) in extra.as_object().unwrap() {
            node[field] = value.clone();
        }
        node
    }

    fn parses(node: Value) -> bool {
        serde_json::from_value::<Node<FileRefs>>(node).is_ok()
    }

    #[test]
    fn a1a_answer_type_restrictions() {
        let cases = [
            ("single_choice", json!({"choices": ["one"]}), true),
            ("multi_choice", json!({"choices": ["one", "two"]}), true),
            ("text", json!({"choices": ["one"]}), false),
            ("single_choice", json!({}), false),
            ("single_choice", json!({"choices": []}), false),
            ("single_choice", json!({"choices": ["one", "one"]}), false),
            ("identity", json!({"fills_role": "owner_role"}), true),
            ("identity_list", json!({"fills_role": "watchers"}), true),
            ("boolean", json!({"fills_role": "owner_role"}), false),
            ("date", json!({"feeds_milestone": "kickoff"}), true),
            ("identity", json!({"feeds_milestone": "kickoff"}), false),
        ];
        for (answer_type, extra, ok) in cases {
            assert_eq!(
                parses(decision(answer_type, &extra)),
                ok,
                "{answer_type} with {extra}"
            );
        }
    }

    #[test]
    fn a_decision_needs_its_prompt_and_answer_type() {
        for missing in ["prompt", "answer_type"] {
            let mut node = decision("text", &json!({}));
            node.as_object_mut().unwrap().remove(missing);
            let error = serde_json::from_value::<Node<FileRefs>>(node).unwrap_err();
            assert!(error.to_string().contains(missing), "{error}");
        }
    }

    #[test]
    fn the_payload_carries_what_the_kind_has() {
        let node: Node<FileRefs> =
            serde_json::from_value(decision("identity", &json!({"fills_role": "owner_role"})))
                .unwrap();
        assert_eq!(node.kind(), NodeKind::Decision);
        let Payload::Decision(decision) = &node.payload else {
            panic!("a decision")
        };
        assert!(
            matches!(&decision.answer, AnswerSpec::Identity { fills_role: Some(role) } if role.as_str() == "owner_role")
        );
        assert_eq!(node.effective_weight(), Weight::DEFAULT);
        let group: Node<FileRefs> =
            serde_json::from_value(json!({"id": "g", "kind": "group", "title": "G"})).unwrap();
        assert_eq!(group.effective_weight(), Weight::GROUP_DEFAULT);
        assert_eq!(group.payload, Payload::Group(Group::default()));
    }

    #[test]
    fn defaults_are_not_written() {
        // Flags at their defaults read back the same and are left out when written, so a
        // canonical file has one spelling per node.
        let explicit =
            json!({"id": "g", "kind": "group", "title": "G", "gates": true, "closes": true});
        let node: Node<FileRefs> = serde_json::from_value(explicit).unwrap();
        assert_eq!(
            serde_json::to_value(&node).unwrap(),
            json!({"id": "g", "kind": "group", "title": "G"})
        );
        let rule = json!({"id": "a", "kind": "action", "title": "A", "due_by": {"after": "kickoff", "offset": 0}});
        let node: Node<FileRefs> = serde_json::from_value(rule).unwrap();
        assert_eq!(
            serde_json::to_value(&node).unwrap()["due_by"],
            json!({"after": "kickoff"})
        );
    }

    #[test]
    fn date_rules_read_as_a8_writes_them() {
        let cases = [
            (json!({"before": "meeting", "offset": 14}), true),
            (json!({"after": ["kickoff", "journey.created_at"]}), true),
            (json!({"before": "a", "after": "b"}), false),
            (json!({"offset": 3}), false),
            (json!({"before": "a", "offset": 366}), false),
            (json!({"before": []}), false),
        ];
        for (rule, ok) in cases {
            let node = json!({"id": "a", "kind": "action", "title": "A", "not_before": rule});
            assert_eq!(parses(node), ok, "{rule}");
        }
    }

    #[test]
    fn participations_read_a_role_or_a_list_of_identities() {
        let node = json!({"id": "a", "kind": "action", "title": "A",
            "participations": {"owner": "owner_role", "informed": ["who_one"], "reviewer": []}});
        let node: Node<FileRefs> = serde_json::from_value(node).unwrap();
        let sources: Vec<bool> = node
            .participations
            .as_map()
            .values()
            .map(|source| matches!(source, ParticipationSource::Role(_)))
            .collect();
        assert_eq!(sources, [false, true, false]);
    }

    #[test]
    fn resources_at_and_past_their_limit() {
        let resources = |count: u32| -> Vec<serde_json::Value> {
            (0..count)
                .map(|index| json!({"tip": format!("Tip {index}.")}))
                .collect()
        };
        let limit = crate::Limit::ResourceCountPerNode.max();
        assert!(parses(
            json!({"id": "a", "kind": "action", "title": "A", "resources": resources(limit)})
        ));
        let past =
            json!({"id": "a", "kind": "action", "title": "A", "resources": resources(limit + 1)});
        let error = serde_json::from_value::<Node<FileRefs>>(past).unwrap_err();
        assert!(
            error
                .to_string()
                .contains(crate::Limit::ResourceCountPerNode.name()),
            "{error}"
        );
    }

    #[test]
    fn explicit_edges_at_and_past_their_limit() {
        let requires =
            |count: u32| -> Vec<String> { (0..count).map(|index| format!("n{index}")).collect() };
        let limit = crate::Limit::EdgeCountPerNode.max();
        assert!(parses(
            json!({"id": "a", "kind": "action", "title": "A", "requires": requires(limit)})
        ));
        let past =
            json!({"id": "a", "kind": "action", "title": "A", "requires": requires(limit + 1)});
        let error = serde_json::from_value::<Node<FileRefs>>(past).unwrap_err();
        assert!(
            error
                .to_string()
                .contains(crate::Limit::EdgeCountPerNode.name()),
            "{error}"
        );
    }
}
