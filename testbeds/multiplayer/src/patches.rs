//! The patches the testbed sends, written as the YAML a person or agent would write and
//! parsed by the schema crate, so every one is a patch the API documents.
//!
//! Three kinds land on the shared journey or the deployment: a note (`annotate`), which
//! touches only its own annotation, so a stale one is always safe to retry (H5); a rename
//! of one of a few nodes (`rename`), which overlaps another rename of the same node, so
//! some conflicts are surfaced; and an entity merge (`merge`), which moves the deployment
//! and ticks every view watching it (H6, E6).

use std::fmt::Write as _;

use cairn_schema::{EntityKey, JourneyId, NodeKey, Patch, Revision};

/// What a client drafts, before it is a patch at some base revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Draft {
    /// A note on the journey with this annotation key.
    Annotate { key: String },
    /// A new title for a node.
    Rename { node: NodeKey, title: String },
    /// Merge one entity into another.
    Merge {
        survivor: EntityKey,
        merged: EntityKey,
    },
}

impl Draft {
    /// The patch `id` makes of this draft at `base`, the revision of its domain the client
    /// last fetched.
    pub fn patch(&self, id: &str, journey: &JourneyId, base: Revision) -> Patch {
        let base = base.get();
        match self {
            Draft::Annotate { key } => parse(&format!(
                "id: {id}\ntarget: {{journey: {journey}}}\nbase_revision: {base}\nmutations:\n\
                 - op: add_annotation\n  annotation: {{key: {key}, note: Seen.}}\n"
            )),
            Draft::Rename { node, title } => parse(&format!(
                "id: {id}\ntarget: {{journey: {journey}}}\nbase_revision: {base}\nmutations:\n\
                 - op: set_node_field\n  node: {node}\n  value: {{title: {title}}}\n"
            )),
            Draft::Merge { survivor, merged } => parse(&format!(
                "id: {id}\ntarget: deployment\nbase_revision: {base}\nmutations:\n\
                 - op: merge_entities\n  survivor: {survivor}\n  merged: {merged}\n  journeys: {{}}\n"
            )),
        }
    }

    /// Whether it patches the deployment rather than the journey.
    pub fn is_deployment(&self) -> bool {
        matches!(self, Draft::Merge { .. })
    }
}

/// The shared journey.
pub fn journey() -> JourneyId {
    key("j_shared")
}

/// The shared journey's node `index`.
pub fn node(index: u32) -> NodeKey {
    key(&format!("n_{index}"))
}

/// Entity pair `index`, as (survivor, merged).
pub fn merge_pair(index: u32) -> (EntityKey, EntityKey) {
    (
        key(&format!("e_survivor_{index}")),
        key(&format!("e_merged_{index}")),
    )
}

/// Creates the shared journey with `nodes` actions at the top level.
pub fn create_journey(nodes: u32) -> Patch {
    let mut yaml = format!(
        "id: p_setup_journey\ntarget: {{journey: {}}}\nbase_revision: 0\nmutations:\n\
         - op: create_journey\n  name: Shared\n",
        journey()
    );
    for index in 0..nodes {
        let _ = writeln!(
            yaml,
            "- op: add_node\n  node: {{key: {}, id: s{index}, kind: action, title: Step {index}}}",
            node(index)
        );
    }
    parse(&yaml)
}

/// Creates `pairs` pairs of entities for merges, in one deployment patch.
pub fn create_entities(pairs: u32) -> Patch {
    let mut yaml =
        "id: p_setup_entities\ntarget: deployment\nbase_revision: 0\nmutations:\n".to_owned();
    for index in 0..pairs {
        let (survivor, merged) = merge_pair(index);
        for (entity, name) in [(survivor, "Survivor"), (merged, "Merged")] {
            let _ = writeln!(
                yaml,
                "- op: create_entity\n  entity: {{key: {entity}, name: {name} {index}}}"
            );
        }
    }
    parse(&yaml)
}

fn parse(yaml: &str) -> Patch {
    match cairn_schema::from_yaml(yaml) {
        Ok(patch) => patch,
        Err(error) => unreachable!("the testbed writes valid patches: {error}\n{yaml}"),
    }
}

fn key<T: std::str::FromStr<Err: std::fmt::Debug>>(text: &str) -> T {
    match text.parse() {
        Ok(key) => key,
        Err(error) => unreachable!("{text:?} is a valid key: {error:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_schema::{Domain, PatchTarget};

    /// Every draft makes a patch to the domain it names, at the base it is given, and the
    /// setup patches create what the drafts refer to.
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
        assert_eq!(
            create_journey(3).target,
            PatchTarget::Journey(journey()),
            "setup creates the shared journey"
        );
        assert_eq!(create_entities(2).mutations.as_slice().len(), 4);
    }
}
