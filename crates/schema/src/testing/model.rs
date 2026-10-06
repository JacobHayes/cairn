//! Strategies for the reference forms, conditions, and attachments.

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;

use super::{
    arb_attachment_key, arb_kind_key, arb_markdown, arb_node_key, arb_path, arb_role_key, arb_slug,
    arb_timestamp, arb_title, arb_url, arb_user_id, parsed,
};

use crate::attachment::{
    Annotation, AnnotationBody, AnnotationContent, JourneyField, MessageTemplate, Resource,
    ResourceContent, Segment,
};
use crate::collections::{BoundedSet, BoundedVec, HasKey, Keyed, LimitOf};
use crate::condition::{Clause, Comparison, Condition, ConditionValue, Membership};
use crate::refs::{FileRefs, KeyRefs, References};
use crate::{AttachmentKey, KindKey, NodeKey, RoleKey};

/// The strategies one document form needs for its references and keys.
pub trait ArbRefs: References {
    /// A reference to a node.
    fn arb_node_ref() -> BoxedStrategy<Self::Node>;
    /// A reference to a role.
    fn arb_role_ref() -> BoxedStrategy<Self::Role>;
    /// A reference to a participation kind.
    fn arb_kind_ref() -> BoxedStrategy<Self::Kind>;
    /// A node's own key slot.
    fn arb_node_key_slot() -> BoxedStrategy<Self::NodeKey>;
    /// A role's own key slot.
    fn arb_role_key_slot() -> BoxedStrategy<Self::RoleKey>;
    /// A kind's own key slot.
    fn arb_kind_key_slot() -> BoxedStrategy<Self::KindKey>;
    /// A resource's own key slot.
    fn arb_attachment_key_slot() -> BoxedStrategy<Self::AttachmentKey>;
}

impl ArbRefs for FileRefs {
    fn arb_node_ref() -> BoxedStrategy<crate::Path> {
        arb_path().boxed()
    }
    fn arb_role_ref() -> BoxedStrategy<crate::Slug> {
        arb_slug().boxed()
    }
    fn arb_kind_ref() -> BoxedStrategy<crate::Slug> {
        arb_slug().boxed()
    }
    fn arb_node_key_slot() -> BoxedStrategy<Option<NodeKey>> {
        prop::option::of(arb_node_key()).boxed()
    }
    fn arb_role_key_slot() -> BoxedStrategy<Option<RoleKey>> {
        prop::option::of(arb_role_key()).boxed()
    }
    fn arb_kind_key_slot() -> BoxedStrategy<Option<KindKey>> {
        prop::option::of(arb_kind_key()).boxed()
    }
    fn arb_attachment_key_slot() -> BoxedStrategy<Option<AttachmentKey>> {
        prop::option::of(arb_attachment_key()).boxed()
    }
}

impl ArbRefs for KeyRefs {
    fn arb_node_ref() -> BoxedStrategy<NodeKey> {
        arb_node_key().boxed()
    }
    fn arb_role_ref() -> BoxedStrategy<RoleKey> {
        arb_role_key().boxed()
    }
    fn arb_kind_ref() -> BoxedStrategy<KindKey> {
        arb_kind_key().boxed()
    }
    fn arb_node_key_slot() -> BoxedStrategy<NodeKey> {
        arb_node_key().boxed()
    }
    fn arb_role_key_slot() -> BoxedStrategy<RoleKey> {
        arb_role_key().boxed()
    }
    fn arb_kind_key_slot() -> BoxedStrategy<KindKey> {
        arb_kind_key().boxed()
    }
    fn arb_attachment_key_slot() -> BoxedStrategy<AttachmentKey> {
        arb_attachment_key().boxed()
    }
}

pub(super) fn bounded_set<T: Ord + std::fmt::Debug, L: LimitOf>(items: Vec<T>) -> BoundedSet<T, L> {
    let unique: BTreeSet<T> = items.into_iter().collect();
    match BoundedSet::new(unique) {
        Ok(set) => set,
        Err(error) => panic!("strategy exceeded a limit: {error}"),
    }
}

pub(super) fn bounded_vec<T, L: LimitOf>(items: Vec<T>) -> BoundedVec<T, L> {
    match BoundedVec::new(items) {
        Ok(list) => list,
        Err(error) => panic!("strategy exceeded a limit: {error}"),
    }
}

pub(super) fn keyed<V: HasKey, L: LimitOf>(values: Vec<V>) -> Keyed<V, L> {
    let mut unique = BTreeMap::new();
    for value in values {
        unique.entry(value.key().clone()).or_insert(value);
    }
    match Keyed::new(unique.into_values()) {
        Ok(map) => map,
        Err(error) => panic!("strategy exceeded a limit: {error}"),
    }
}

/// A value a condition compares with.
pub fn arb_condition_value() -> impl Strategy<Value = ConditionValue> {
    prop_oneof![
        any::<bool>().prop_map(ConditionValue::Boolean),
        "[a-z][a-z0-9-]{0,8}".prop_map(|text| ConditionValue::Text(parsed(&text))),
    ]
}

/// A condition within the depth and clause limits.
pub fn arb_condition<R: ArbRefs>() -> impl Strategy<Value = Condition<R>> {
    let comparison = || {
        (R::arb_node_ref(), arb_condition_value())
            .prop_map(|(decision, value)| Comparison { decision, value })
    };
    let leaf = prop_oneof![
        comparison().prop_map(Clause::Equals),
        comparison().prop_map(Clause::NotEquals),
        comparison().prop_map(Clause::Contains),
        (
            R::arb_node_ref(),
            prop::collection::vec(arb_condition_value(), 1..4)
        )
            .prop_map(|(decision, values)| Clause::In(Membership {
                decision,
                values: bounded_vec(values)
            })),
        R::arb_node_ref().prop_map(Clause::Answered),
    ];
    // Depth at most 4 and at most 3 children per combination keep clauses under 16.
    leaf.prop_recursive(3, 12, 3, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 1..3).prop_map(Clause::All),
            prop::collection::vec(inner.clone(), 1..3).prop_map(Clause::Any),
            inner.prop_map(|clause| Clause::Not(Box::new(clause))),
        ]
    })
    .prop_filter_map("within the condition limits", |root| {
        Condition::new(root).ok()
    })
}

/// A message template whose text never repeats a placeholder's braces.
pub fn arb_message_template<R: ArbRefs>() -> impl Strategy<Value = MessageTemplate<R>> {
    let placeholder = prop_oneof![
        prop_oneof![
            Just(JourneyField::Name),
            Just(JourneyField::Description),
            Just(JourneyField::CreatedAt),
            Just(JourneyField::Url),
            Just(JourneyField::Status),
        ]
        .prop_map(Segment::Journey),
        R::arb_role_ref().prop_map(Segment::RoleName),
        R::arb_node_ref().prop_map(Segment::Answer),
    ];
    // Text and placeholders alternate, so no two text segments touch (they would read back
    // as one).
    (
        prop::collection::vec((placeholder, "[A-Za-z ,.]{1,8}"), 1..4),
        "[A-Za-z ,.]{0,8}",
    )
        .prop_map(|(pairs, lead)| {
            let mut text = lead;
            for (segment, after) in pairs {
                let rendered = MessageTemplate::<R>::render_segment(&segment);
                text.push_str(&rendered);
                text.push_str(&after);
            }
            parsed(&text)
        })
}

/// A resource.
pub fn arb_resource<R: ArbRefs>() -> impl Strategy<Value = Resource<R>> {
    let content = prop_oneof![
        arb_markdown().prop_map(ResourceContent::Tip),
        arb_url().prop_map(ResourceContent::Template),
        arb_url().prop_map(ResourceContent::Example),
        arb_url().prop_map(ResourceContent::Reference),
        arb_message_template::<R>().prop_map(ResourceContent::MessageDraft),
    ];
    (
        R::arb_attachment_key_slot(),
        prop::option::of(arb_title()),
        content,
    )
        .prop_map(|(key, title, content)| Resource {
            key,
            title,
            content,
        })
}

/// A note or link as written.
pub fn arb_annotation_body() -> impl Strategy<Value = AnnotationBody> {
    let content = prop_oneof![
        arb_markdown().prop_map(AnnotationContent::Note),
        arb_url().prop_map(AnnotationContent::Artifact),
        arb_url().prop_map(AnnotationContent::Reference),
        arb_url().prop_map(AnnotationContent::Conversation),
    ];
    (
        arb_attachment_key(),
        prop::option::of(arb_node_key()),
        prop::option::of(arb_title()),
        content,
    )
        .prop_map(|(key, node, title, content)| AnnotationBody {
            key,
            node,
            title,
            content,
        })
}

/// A note or link, attributed.
pub fn arb_annotation() -> impl Strategy<Value = Annotation> {
    (
        arb_annotation_body(),
        arb_user_id(),
        arb_timestamp(),
        prop::option::of(arb_timestamp()),
    )
        .prop_map(|(body, created_by, created_at, edited_at)| Annotation {
            body,
            created_by,
            created_at,
            edited_at,
        })
}
