//! Strategies for nodes: payloads, date rules, participations.

use proptest::prelude::*;

use super::arb_entity_key;
use super::model::{bounded_set, bounded_vec};
use super::{
    ArbRefs, arb_condition, arb_days, arb_markdown, arb_resource, arb_slug, arb_title, arb_weight,
};

use crate::collections::OneOrMany;
use crate::field::{NodeField, NodeFieldValue};
use crate::node::{
    Action, AnswerSpec, Choice, Choices, DateRule, DateSource, Decision, Deliverable, Direction,
    Group, Milestone, Node, ParticipationSource, Participations, Payload,
};

/// A date rule.
pub fn arb_date_rule<R: ArbRefs>() -> impl Strategy<Value = DateRule<R>> {
    let source = prop_oneof![1 => Just(DateSource::CreatedAt), 3 => R::arb_node_ref().prop_map(DateSource::Node)];
    (
        prop_oneof![Just(Direction::Before), Just(Direction::After)],
        prop::collection::btree_set(source, 1..3),
        arb_days(),
    )
        .prop_map(|(direction, sources, offset)| DateRule {
            direction,
            sources: match OneOrMany::new(sources) {
                Ok(sources) => sources,
                Err(error) => panic!("strategy produced bad sources: {error}"),
            },
            offset,
        })
}

fn arb_choices() -> impl Strategy<Value = Choices> {
    prop::collection::btree_map(arb_slug(), prop::option::of(arb_title()), 1..5).prop_map(
        |choices| {
            let list = choices
                .into_iter()
                .map(|(id, title)| Choice { id, title })
                .collect();
            match Choices::try_from(bounded_vec::<
                Choice,
                crate::collections::ChoiceCountPerDecision,
            >(list))
            {
                Ok(choices) => choices,
                Err(error) => panic!("strategy produced bad choices: {error}"),
            }
        },
    )
}

fn arb_answer_spec<R: ArbRefs>() -> impl Strategy<Value = AnswerSpec<R>> {
    prop_oneof![
        Just(AnswerSpec::Boolean),
        arb_choices().prop_map(AnswerSpec::SingleChoice),
        arb_choices().prop_map(AnswerSpec::MultiChoice),
        Just(AnswerSpec::Text),
        prop::option::of(R::arb_node_ref())
            .prop_map(|feeds_milestone| AnswerSpec::Date { feeds_milestone }),
        prop::option::of(R::arb_role_ref())
            .prop_map(|fills_role| AnswerSpec::Entity { fills_role }),
        prop::option::of(R::arb_role_ref())
            .prop_map(|fills_role| AnswerSpec::EntityList { fills_role }),
    ]
}

/// A node's kind-specific payload.
pub fn arb_payload<R: ArbRefs>() -> impl Strategy<Value = Payload<R>> {
    prop_oneof![
        (
            arb_markdown(),
            prop::option::of(arb_markdown()),
            arb_answer_spec::<R>()
        )
            .prop_map(|(prompt, help, answer)| Payload::Decision(Decision {
                prompt,
                help,
                answer
            })),
        (prop::option::of(arb_days()), any::<bool>(), any::<bool>()).prop_map(
            |(estimate, placeholder, requires_artifact)| {
                Payload::Deliverable(Deliverable {
                    estimate,
                    placeholder,
                    requires_artifact,
                })
            }
        ),
        (prop::option::of(arb_days()), any::<bool>()).prop_map(|(estimate, placeholder)| {
            Payload::Action(Action {
                estimate,
                placeholder,
            })
        }),
        (any::<bool>(), any::<bool>()).prop_map(|(is_final, auto_reach)| Payload::Milestone(
            Milestone {
                is_final,
                auto_reach
            }
        )),
        (
            prop::option::of(R::arb_node_ref()),
            prop::option::of(R::arb_node_ref()),
            any::<bool>(),
            any::<bool>()
        )
            .prop_map(
                |(opens_at, closes_at, gates, closes)| Payload::Group(Group {
                    opens_at,
                    closes_at,
                    gates,
                    closes
                })
            ),
    ]
}

/// A node's participations.
pub fn arb_participations<R: ArbRefs>() -> impl Strategy<Value = Participations<R>> {
    let source = prop_oneof![
        R::arb_role_ref().prop_map(ParticipationSource::Role),
        arb_entity_set().prop_map(ParticipationSource::Entities),
    ];
    prop::collection::btree_map(R::arb_kind_ref(), source, 0..3).prop_map(|map| {
        match Participations::try_from(map) {
            Ok(participations) => participations,
            Err(error) => panic!("strategy exceeded a limit: {error}"),
        }
    })
}

/// A node.
pub fn arb_node<R: ArbRefs>() -> impl Strategy<Value = Node<R>> {
    let identity = (
        R::arb_node_key_slot(),
        arb_slug(),
        prop::option::of(R::arb_node_ref()),
        arb_title(),
        prop::option::of(arb_markdown()),
        prop::option::of(arb_weight()),
    );
    let links = (
        prop::collection::vec(R::arb_node_ref(), 0..3),
        prop::option::of(arb_condition::<R>()),
        prop::option::of(arb_date_rule::<R>()),
        prop::option::of(arb_date_rule::<R>()),
        arb_participations::<R>(),
        prop::collection::vec(arb_resource::<R>(), 0..2),
    );
    (identity, links, arb_payload::<R>()).prop_map(
        |(
            (key, id, parent, title, description, weight),
            (requires, relevant_when, due_by, not_before, participations, resources),
            payload,
        )| Node {
            key,
            id,
            parent,
            title,
            description,
            weight,
            requires: bounded_set(requires),
            relevant_when,
            due_by,
            not_before,
            participations,
            resources,
            payload,
        },
    )
}

/// A node field name.
pub fn arb_node_field() -> impl Strategy<Value = NodeField> {
    prop::sample::select(NodeField::ALL.to_vec())
}

/// A set of explicit entities, possibly empty.
pub fn arb_entity_set() -> impl Strategy<Value = crate::node::EntitySet> {
    prop::collection::vec(arb_entity_key(), 0..4).prop_map(bounded_set)
}

/// A new value for a node field.
pub fn arb_node_field_value<R: ArbRefs>() -> impl Strategy<Value = NodeFieldValue<R>> {
    let optional_node = || prop::option::of(R::arb_node_ref());
    prop_oneof![
        arb_slug().prop_map(NodeFieldValue::Id),
        optional_node().prop_map(NodeFieldValue::Parent),
        arb_title().prop_map(NodeFieldValue::Title),
        prop::option::of(arb_markdown()).prop_map(NodeFieldValue::Description),
        prop::option::of(arb_weight()).prop_map(NodeFieldValue::Weight),
        prop::option::of(arb_condition::<R>()).prop_map(NodeFieldValue::RelevantWhen),
        prop::option::of(arb_date_rule::<R>()).prop_map(NodeFieldValue::DueBy),
        prop::option::of(arb_date_rule::<R>()).prop_map(NodeFieldValue::NotBefore),
        prop::option::of(arb_days()).prop_map(NodeFieldValue::Estimate),
        any::<bool>().prop_map(NodeFieldValue::Placeholder),
        any::<bool>().prop_map(NodeFieldValue::RequiresArtifact),
        any::<bool>().prop_map(NodeFieldValue::Final),
        any::<bool>().prop_map(NodeFieldValue::AutoReach),
        optional_node().prop_map(NodeFieldValue::OpensAt),
        optional_node().prop_map(NodeFieldValue::ClosesAt),
        any::<bool>().prop_map(NodeFieldValue::Gates),
        any::<bool>().prop_map(NodeFieldValue::Closes),
        arb_markdown().prop_map(NodeFieldValue::Prompt),
        prop::option::of(arb_markdown()).prop_map(NodeFieldValue::Help),
        arb_choices().prop_map(NodeFieldValue::Choices),
        prop::option::of(R::arb_role_ref()).prop_map(NodeFieldValue::FillsRole),
        optional_node().prop_map(NodeFieldValue::FeedsMilestone),
    ]
}
