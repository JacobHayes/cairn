//! The patches the testbed sends, built as typed schema values the way the service takes
//! them, so a change to the patch schema fails the testbed's build rather than its run.
//!
//! Three kinds land on the shared journey or the deployment: a note (`annotate`), which
//! touches only its own annotation, so a stale one is always safe to retry (H5); a rename
//! of one of a few nodes (`rename`), which overlaps another rename of the same node, so
//! some conflicts are surfaced; and an identity merge (`merge`), which moves the deployment
//! and ticks every view watching it (H6, E6).

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    Action, AnnotationBody, AnnotationContent, BoundedSet, Identity, IdentityKey, IdentitySet,
    JourneyId, KeyRefs, Mutation, Mutations, Node, NodeFieldValue, NodeKey, Participations, Patch,
    PatchTarget, Payload, Revision,
};

/// What a client drafts, before it is a patch at some base revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Draft {
    /// A note on the journey with this annotation key.
    Annotate { key: String },
    /// A new title for a node.
    Rename { node: NodeKey, title: String },
    /// Merge one identity into another.
    Merge {
        survivor: IdentityKey,
        merged: IdentityKey,
    },
}

impl Draft {
    /// The patch `id` makes of this draft at `base`, the revision of its domain the client
    /// last fetched.
    pub fn patch(&self, id: &str, journey: &JourneyId, base: Revision) -> Patch {
        let (target, mutation) = match self {
            Draft::Annotate { key } => (
                PatchTarget::Journey(journey.clone()),
                Mutation::AddAnnotation {
                    annotation: AnnotationBody {
                        key: parse(key),
                        node: None,
                        title: None,
                        content: AnnotationContent::Note(parse("Seen.")),
                    },
                },
            ),
            Draft::Rename { node, title } => (
                PatchTarget::Journey(journey.clone()),
                Mutation::SetNodeField {
                    node: node.clone(),
                    value: NodeFieldValue::Title(parse(title)),
                },
            ),
            Draft::Merge { survivor, merged } => (
                PatchTarget::Deployment,
                Mutation::MergeIdentities {
                    survivor: survivor.clone(),
                    merged: checked(IdentitySet::new([merged.clone()])),
                    journeys: BTreeMap::new(),
                },
            ),
        };
        patch(id, target, base, vec![mutation])
    }

    /// Whether it patches the deployment rather than the journey.
    pub fn is_deployment(&self) -> bool {
        matches!(self, Draft::Merge { .. })
    }
}

/// The shared journey.
pub fn journey() -> JourneyId {
    parse("j_shared")
}

/// The shared journey's node `index`.
pub fn node(index: u32) -> NodeKey {
    parse(&format!("n_{index}"))
}

/// Identity pair `index`, as (survivor, merged): keys are opaque, so the pair is told apart
/// by its place in the testbed's own data, the survivor first.
pub fn merge_pair(index: u32) -> (IdentityKey, IdentityKey) {
    (
        parse(&format!("who_{}", 2 * index)),
        parse(&format!("who_{}", 2 * index + 1)),
    )
}

/// Creates the shared journey with `nodes` actions at the top level.
pub fn create_journey(nodes: u32) -> Patch {
    let mut mutations = vec![Mutation::CreateJourney {
        name: parse("Shared"),
        description: None,
        owners: cairn_schema::IdentitySet::default(),
        from: None,
    }];
    mutations.extend((0..nodes).map(|index| Mutation::AddNode {
        node: action(node(index), index),
    }));
    patch(
        "p_setup_journey",
        PatchTarget::Journey(journey()),
        Revision::NONE,
        mutations,
    )
}

/// Creates `pairs` pairs of identities for merges, in one deployment patch.
pub fn create_identities(pairs: u32) -> Patch {
    let mutations = (0..pairs)
        .flat_map(|index| {
            let (survivor, merged) = merge_pair(index);
            [(survivor, "Survivor"), (merged, "Merged")].map(|(key, name)| {
                Mutation::CreateIdentity {
                    identity: Identity {
                        key,
                        name: Some(parse(&format!("{name} {index}"))),
                        emails: BTreeSet::new(),
                    },
                }
            })
        })
        .collect();
    patch(
        "p_setup_identities",
        PatchTarget::Deployment,
        Revision::NONE,
        mutations,
    )
}

/// Action node `key`, at the top level with id `s{index}` and title `Step {index}`.
fn action(key: NodeKey, index: u32) -> Node<KeyRefs> {
    Node {
        key,
        id: parse(&format!("s{index}")),
        parent: None,
        title: parse(&format!("Step {index}")),
        description: None,
        weight: None,
        requires: BoundedSet::default(),
        relevant_when: None,
        due_by: None,
        not_before: None,
        participations: Participations::default(),
        resources: Vec::new(),
        payload: Payload::Action(Action::default()),
    }
}

fn patch(id: &str, target: PatchTarget, base: Revision, mutations: Vec<Mutation>) -> Patch {
    Patch {
        id: parse(id),
        target,
        base_revision: base,
        deployment_revision: None,
        mutations: checked(Mutations::new(mutations)),
    }
}

/// The value of a collection the testbed builds within its limits.
fn checked<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => unreachable!("the testbed builds collections within their limits: {error:?}"),
    }
}

/// The key or text `text` spells, which the testbed writes valid.
fn parse<T: std::str::FromStr<Err: std::fmt::Debug>>(text: &str) -> T {
    match text.parse() {
        Ok(value) => value,
        Err(error) => unreachable!("{text:?} is valid: {error:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_schema::Domain;

    /// Every draft makes a patch to the domain it names, at the base it is given, and the
    /// identity setup creates both identities of every pair.
    #[test]
    fn drafts_make_patches_to_their_domains() {
        let base = Revision::try_from(4).unwrap();
        let (survivor, merged) = merge_pair(0);
        let drafts = [
            Draft::Annotate {
                key: "a_c1_2".to_owned(),
            },
            Draft::Rename {
                node: node(1),
                title: "c1 a2".to_owned(),
            },
            Draft::Merge { survivor, merged },
        ];
        for draft in drafts {
            let patch = draft.patch("p_c1_2", &journey(), base);
            assert_eq!(patch.base_revision, base, "{draft:?}");
            let expected = if draft.is_deployment() {
                Domain::Deployment
            } else {
                Domain::Journey(journey())
            };
            assert_eq!(patch.target.domain(), expected, "{draft:?}");
        }
        assert_eq!(create_identities(2).mutations.len(), 4);
    }
}
