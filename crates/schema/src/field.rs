//! A node's individually editable fields (PRD H5, by key and field). A
//! field edit names one of these, so two edits to different fields of one node do not
//! overlap. Changing a node's kind or a decision's answer type replaces the whole node.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::condition::Condition;
use crate::id::Slug;
use crate::node::{Checklist, Choices, DateRule, KindField, Node, Payload};
use crate::number::{Days, Weight};
use crate::refs::References;
use crate::text::{Markdown, Title};

/// A node field, by name.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum NodeField {
    /// The id (a rename).
    Id,
    /// The parent (a move).
    Parent,
    /// The title.
    Title,
    /// The description.
    Description,
    // PRD: B5.
    /// The weight; in a journey, a weight override.
    Weight,
    /// The `relevant_when` condition.
    RelevantWhen,
    /// The `due_by` rule.
    DueBy,
    /// The `not_before` rule.
    NotBefore,
    /// A deliverable's or action's estimate.
    Estimate,
    /// A deliverable's or action's `placeholder` flag.
    Placeholder,
    /// A deliverable's `requires_artifact` flag.
    RequiresArtifact,
    /// A deliverable's or action's `requires_note` flag.
    RequiresNote,
    /// A deliverable's or action's checklist.
    Checklist,
    /// A deliverable's or action's `requires_checklist` flag.
    RequiresChecklist,
    /// A milestone's `final` flag.
    Final,
    /// A milestone's `auto_done` flag.
    AutoDone,
    /// A decision's prompt.
    Prompt,
    /// A decision's help text.
    Help,
    /// A choice decision's choices.
    Choices,
    /// An identity decision's `fills_role`.
    FillsRole,
    /// A date decision's `feeds_milestone`.
    FeedsMilestone,
}

impl NodeField {
    /// The kind-restricted field this is, or none for a field every node has.
    #[must_use]
    pub const fn kind_field(self) -> Option<KindField> {
        Some(match self {
            NodeField::Id
            | NodeField::Parent
            | NodeField::Title
            | NodeField::Description
            | NodeField::Weight
            | NodeField::RelevantWhen
            | NodeField::DueBy
            | NodeField::NotBefore => return None,
            NodeField::Estimate => KindField::Estimate,
            NodeField::Placeholder => KindField::Placeholder,
            NodeField::RequiresArtifact => KindField::RequiresArtifact,
            NodeField::RequiresNote => KindField::RequiresNote,
            NodeField::Checklist => KindField::Checklist,
            NodeField::RequiresChecklist => KindField::RequiresChecklist,
            NodeField::Final => KindField::Final,
            NodeField::AutoDone => KindField::AutoDone,
            NodeField::Prompt => KindField::Prompt,
            NodeField::Help => KindField::Help,
            NodeField::Choices => KindField::Choices,
            NodeField::FillsRole => KindField::FillsRole,
            NodeField::FeedsMilestone => KindField::FeedsMilestone,
        })
    }

    /// Whether a payload has this field, by the kind field table; false for a field every
    /// node has, which the payload does not hold.
    fn allowed_on<R: References>(self, payload: &Payload<R>) -> bool {
        self.kind_field()
            .is_some_and(|field| field.allowed(payload.kind(), payload.answer_type()))
    }

    /// Every field, in declaration order.
    pub const ALL: [NodeField; 21] = [
        NodeField::Id,
        NodeField::Parent,
        NodeField::Title,
        NodeField::Description,
        NodeField::Weight,
        NodeField::RelevantWhen,
        NodeField::DueBy,
        NodeField::NotBefore,
        NodeField::Estimate,
        NodeField::Placeholder,
        NodeField::RequiresArtifact,
        NodeField::RequiresNote,
        NodeField::Checklist,
        NodeField::RequiresChecklist,
        NodeField::Final,
        NodeField::AutoDone,
        NodeField::Prompt,
        NodeField::Help,
        NodeField::Choices,
        NodeField::FillsRole,
        NodeField::FeedsMilestone,
    ];
}

/// A new value for one node field.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields, bound = "")]
#[schemars(bound = "R: References", rename = "NodeFieldValue{R}")]
pub enum NodeFieldValue<R: References> {
    /// A new id.
    Id(Slug),
    /// A new parent, or none to make the node a root.
    Parent(Option<R::Node>),
    /// A new title.
    Title(Title),
    /// A new description, or none.
    Description(Option<Markdown>),
    /// A new weight, or none for the kind's default.
    Weight(Option<Weight>),
    /// A new condition, or none.
    RelevantWhen(Option<Condition<R>>),
    /// A new `due_by` rule, or none.
    DueBy(Option<DateRule<R>>),
    /// A new `not_before` rule, or none.
    NotBefore(Option<DateRule<R>>),
    /// A new estimate, or none.
    Estimate(Option<Days>),
    /// The `placeholder` flag.
    Placeholder(bool),
    /// The `requires_artifact` flag.
    RequiresArtifact(bool),
    /// The `requires_note` flag.
    RequiresNote(bool),
    /// A new checklist; empty clears it.
    Checklist(Checklist),
    /// The `requires_checklist` flag.
    RequiresChecklist(bool),
    /// The `final` flag.
    Final(bool),
    /// The `auto_done` flag.
    AutoDone(bool),
    /// A new prompt.
    Prompt(Markdown),
    /// New help text, or none.
    Help(Option<Markdown>),
    /// New choices.
    Choices(Choices),
    /// A new `fills_role`, or none.
    FillsRole(Option<R::Role>),
    /// A new `feeds_milestone`, or none.
    FeedsMilestone(Option<R::Node>),
}

impl<R: References> NodeFieldValue<R> {
    /// The field this value is for.
    #[must_use]
    pub fn field(&self) -> NodeField {
        match self {
            NodeFieldValue::Id(_) => NodeField::Id,
            NodeFieldValue::Parent(_) => NodeField::Parent,
            NodeFieldValue::Title(_) => NodeField::Title,
            NodeFieldValue::Description(_) => NodeField::Description,
            NodeFieldValue::Weight(_) => NodeField::Weight,
            NodeFieldValue::RelevantWhen(_) => NodeField::RelevantWhen,
            NodeFieldValue::DueBy(_) => NodeField::DueBy,
            NodeFieldValue::NotBefore(_) => NodeField::NotBefore,
            NodeFieldValue::Estimate(_) => NodeField::Estimate,
            NodeFieldValue::Placeholder(_) => NodeField::Placeholder,
            NodeFieldValue::RequiresArtifact(_) => NodeField::RequiresArtifact,
            NodeFieldValue::RequiresNote(_) => NodeField::RequiresNote,
            NodeFieldValue::Checklist(_) => NodeField::Checklist,
            NodeFieldValue::RequiresChecklist(_) => NodeField::RequiresChecklist,
            NodeFieldValue::Final(_) => NodeField::Final,
            NodeFieldValue::AutoDone(_) => NodeField::AutoDone,
            NodeFieldValue::Prompt(_) => NodeField::Prompt,
            NodeFieldValue::Help(_) => NodeField::Help,
            NodeFieldValue::Choices(_) => NodeField::Choices,
            NodeFieldValue::FillsRole(_) => NodeField::FillsRole,
            NodeFieldValue::FeedsMilestone(_) => NodeField::FeedsMilestone,
        }
    }

    /// The node's current value of this field, when its kind has the field.
    #[must_use]
    pub fn read(field: NodeField, node: &Node<R>) -> Option<Self> {
        Some(match field {
            NodeField::Id => NodeFieldValue::Id(node.id.clone()),
            NodeField::Parent => NodeFieldValue::Parent(node.parent.clone()),
            NodeField::Title => NodeFieldValue::Title(node.title.clone()),
            NodeField::Description => NodeFieldValue::Description(node.description.clone()),
            NodeField::Weight => NodeFieldValue::Weight(node.weight),
            NodeField::RelevantWhen => NodeFieldValue::RelevantWhen(node.relevant_when.clone()),
            NodeField::DueBy => NodeFieldValue::DueBy(node.due_by.clone()),
            NodeField::NotBefore => NodeFieldValue::NotBefore(node.not_before.clone()),
            NodeField::Estimate
            | NodeField::Placeholder
            | NodeField::RequiresArtifact
            | NodeField::RequiresNote
            | NodeField::Checklist
            | NodeField::RequiresChecklist
            | NodeField::Final
            | NodeField::AutoDone
            | NodeField::Prompt
            | NodeField::Help
            | NodeField::Choices
            | NodeField::FillsRole
            | NodeField::FeedsMilestone => return Self::read_payload(field, &node.payload),
        })
    }

    /// The value of a field that deliverables and actions carry; none for any other field.
    fn read_work(field: NodeField, payload: &Payload<R>) -> Option<Self> {
        match (field, payload) {
            (NodeField::Estimate, Payload::Deliverable(work)) => {
                Some(NodeFieldValue::Estimate(work.estimate))
            }
            (NodeField::Estimate, Payload::Action(work)) => {
                Some(NodeFieldValue::Estimate(work.estimate))
            }
            (NodeField::Placeholder, Payload::Deliverable(work)) => {
                Some(NodeFieldValue::Placeholder(work.placeholder))
            }
            (NodeField::Placeholder, Payload::Action(work)) => {
                Some(NodeFieldValue::Placeholder(work.placeholder))
            }
            (NodeField::RequiresArtifact, Payload::Deliverable(work)) => {
                Some(NodeFieldValue::RequiresArtifact(work.requires_artifact))
            }
            (NodeField::RequiresNote, Payload::Deliverable(work)) => {
                Some(NodeFieldValue::RequiresNote(work.requires_note))
            }
            (NodeField::RequiresNote, Payload::Action(work)) => {
                Some(NodeFieldValue::RequiresNote(work.requires_note))
            }
            (NodeField::Checklist, Payload::Deliverable(work)) => {
                Some(NodeFieldValue::Checklist(work.checklist.clone()))
            }
            (NodeField::Checklist, Payload::Action(work)) => {
                Some(NodeFieldValue::Checklist(work.checklist.clone()))
            }
            (NodeField::RequiresChecklist, Payload::Deliverable(work)) => {
                Some(NodeFieldValue::RequiresChecklist(work.requires_checklist))
            }
            (NodeField::RequiresChecklist, Payload::Action(work)) => {
                Some(NodeFieldValue::RequiresChecklist(work.requires_checklist))
            }
            _ => None,
        }
    }

    fn read_payload(field: NodeField, payload: &Payload<R>) -> Option<Self> {
        use crate::node::AnswerSpec;
        if !field.allowed_on(payload) {
            return None;
        }
        if let Some(value) = Self::read_work(field, payload) {
            return Some(value);
        }
        match (field, payload) {
            (NodeField::Final, Payload::Milestone(milestone)) => {
                Some(NodeFieldValue::Final(milestone.is_final))
            }
            (NodeField::AutoDone, Payload::Milestone(milestone)) => {
                Some(NodeFieldValue::AutoDone(milestone.auto_done))
            }
            (NodeField::Prompt, Payload::Decision(decision)) => {
                Some(NodeFieldValue::Prompt(decision.prompt.clone()))
            }
            (NodeField::Help, Payload::Decision(decision)) => {
                Some(NodeFieldValue::Help(decision.help.clone()))
            }
            (NodeField::Choices, Payload::Decision(decision)) => match &decision.answer {
                AnswerSpec::SingleChoice(choices) | AnswerSpec::MultiChoice(choices) => {
                    Some(NodeFieldValue::Choices(choices.clone()))
                }
                _ => unreachable!("the kind field table allows choices only on a choice"),
            },
            (NodeField::FillsRole, Payload::Decision(decision)) => match &decision.answer {
                AnswerSpec::Identity { fills_role } | AnswerSpec::IdentityList { fills_role } => {
                    Some(NodeFieldValue::FillsRole(fills_role.clone()))
                }
                _ => unreachable!("the kind field table allows fills_role only on an identity"),
            },
            (NodeField::FeedsMilestone, Payload::Decision(decision)) => match &decision.answer {
                AnswerSpec::Date { feeds_milestone } => {
                    Some(NodeFieldValue::FeedsMilestone(feeds_milestone.clone()))
                }
                _ => unreachable!("the kind field table allows feeds_milestone only on a date"),
            },
            _ => unreachable!("the kind field table allows only the fields read above"),
        }
    }

    /// Writes the value into `node`. Returns false, leaving the node unchanged, when the
    /// node's kind (or answer type) does not have the field: the engine reports that as a
    /// violation rather than this function deciding how.
    #[must_use]
    pub fn write(self, node: &mut Node<R>) -> bool {
        match self {
            NodeFieldValue::Id(id) => node.id = id,
            NodeFieldValue::Parent(parent) => node.parent = parent,
            NodeFieldValue::Title(title) => node.title = title,
            NodeFieldValue::Description(description) => node.description = description,
            NodeFieldValue::Weight(weight) => node.weight = weight,
            NodeFieldValue::RelevantWhen(condition) => node.relevant_when = condition,
            NodeFieldValue::DueBy(rule) => node.due_by = rule,
            NodeFieldValue::NotBefore(rule) => node.not_before = rule,
            other => return other.write_payload(&mut node.payload),
        }
        true
    }

    /// Writes a field that deliverables and actions carry; any other field is handed back.
    fn write_work(self, payload: &mut Payload<R>) -> Result<(), Self> {
        match (self, payload) {
            (NodeFieldValue::Estimate(days), Payload::Deliverable(work)) => work.estimate = days,
            (NodeFieldValue::Estimate(days), Payload::Action(work)) => work.estimate = days,
            (NodeFieldValue::Placeholder(flag), Payload::Deliverable(work)) => {
                work.placeholder = flag;
            }
            (NodeFieldValue::Placeholder(flag), Payload::Action(work)) => work.placeholder = flag,
            (NodeFieldValue::RequiresArtifact(flag), Payload::Deliverable(work)) => {
                work.requires_artifact = flag;
            }
            (NodeFieldValue::RequiresNote(flag), Payload::Deliverable(work)) => {
                work.requires_note = flag;
            }
            (NodeFieldValue::RequiresNote(flag), Payload::Action(work)) => {
                work.requires_note = flag;
            }
            (NodeFieldValue::Checklist(checklist), Payload::Deliverable(work)) => {
                work.checklist = checklist;
            }
            (NodeFieldValue::Checklist(checklist), Payload::Action(work)) => {
                work.checklist = checklist;
            }
            (NodeFieldValue::RequiresChecklist(flag), Payload::Deliverable(work)) => {
                work.requires_checklist = flag;
            }
            (NodeFieldValue::RequiresChecklist(flag), Payload::Action(work)) => {
                work.requires_checklist = flag;
            }
            (other, _) => return Err(other),
        }
        Ok(())
    }

    fn write_payload(self, payload: &mut Payload<R>) -> bool {
        use crate::node::AnswerSpec;
        if !self.field().allowed_on(payload) {
            return false;
        }
        let Err(this) = self.write_work(payload) else {
            return true;
        };
        match (this, payload) {
            (NodeFieldValue::Final(flag), Payload::Milestone(milestone)) => {
                milestone.is_final = flag;
            }
            (NodeFieldValue::AutoDone(flag), Payload::Milestone(milestone)) => {
                milestone.auto_done = flag;
            }
            (NodeFieldValue::Prompt(prompt), Payload::Decision(decision)) => {
                decision.prompt = prompt;
            }
            (NodeFieldValue::Help(help), Payload::Decision(decision)) => decision.help = help,
            (NodeFieldValue::Choices(new), Payload::Decision(decision)) => {
                match &mut decision.answer {
                    AnswerSpec::SingleChoice(choices) | AnswerSpec::MultiChoice(choices) => {
                        *choices = new;
                    }
                    _ => unreachable!("the kind field table allows choices only on a choice"),
                }
            }
            (NodeFieldValue::FillsRole(role), Payload::Decision(decision)) => {
                match &mut decision.answer {
                    AnswerSpec::Identity { fills_role }
                    | AnswerSpec::IdentityList { fills_role } => {
                        *fills_role = role;
                    }
                    _ => unreachable!("the kind field table allows fills_role only on an identity"),
                }
            }
            (NodeFieldValue::FeedsMilestone(milestone), Payload::Decision(decision)) => {
                match &mut decision.answer {
                    AnswerSpec::Date { feeds_milestone } => {
                        *feeds_milestone = milestone;
                    }
                    _ => unreachable!("the kind field table allows feeds_milestone only on a date"),
                }
            }
            _ => unreachable!("the kind field table allows only the fields written above"),
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refs::FileRefs;
    use serde_json::json;

    fn node(value: serde_json::Value) -> Node<FileRefs> {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn a_field_reads_and_writes_only_where_the_kind_has_it() {
        let deliverable =
            node(json!({"id": "d", "kind": "deliverable", "title": "D", "estimate": 2}));
        let milestone = node(json!({"id": "m", "kind": "milestone", "title": "M"}));
        let estimate = NodeFieldValue::<FileRefs>::read(NodeField::Estimate, &deliverable).unwrap();
        assert_eq!(estimate.field(), NodeField::Estimate);
        assert_eq!(
            NodeFieldValue::<FileRefs>::read(NodeField::Estimate, &milestone),
            None
        );

        let mut changed = deliverable.clone();
        let three = NodeFieldValue::Estimate(Some(Days::try_from(3).unwrap()));
        assert!(three.clone().write(&mut changed));
        assert_eq!(
            NodeFieldValue::read(NodeField::Estimate, &changed),
            Some(three.clone())
        );
        let mut unchanged = milestone.clone();
        assert!(!three.write(&mut unchanged));
        assert_eq!(unchanged, milestone);
    }

    #[test]
    fn answer_fields_follow_the_answer_type() {
        let identity = node(
            json!({"id": "e", "kind": "decision", "title": "E", "prompt": "Who?", "answer_type": "identity"}),
        );
        let text = node(
            json!({"id": "t", "kind": "decision", "title": "T", "prompt": "What?", "answer_type": "text"}),
        );
        let role = NodeFieldValue::FillsRole(Some("owner_role".parse().unwrap()));
        let mut filled = identity.clone();
        assert!(role.clone().write(&mut filled));
        assert_eq!(
            NodeFieldValue::read(NodeField::FillsRole, &filled),
            Some(role.clone())
        );
        let mut unchanged = text.clone();
        assert!(!role.write(&mut unchanged));
        assert_eq!(
            NodeFieldValue::<FileRefs>::read(NodeField::Choices, &text),
            None
        );
    }

    #[test]
    fn field_values_serialize_by_field_name() {
        let value: NodeFieldValue<FileRefs> =
            serde_json::from_value(json!({"title": "Renamed"})).unwrap();
        assert_eq!(value.field(), NodeField::Title);
        assert_eq!(
            serde_json::to_value(NodeField::RequiresArtifact).unwrap(),
            json!("requires_artifact")
        );
        assert!(
            serde_json::from_value::<NodeFieldValue<FileRefs>>(json!({"kind": "group"})).is_err()
        );
    }
}
