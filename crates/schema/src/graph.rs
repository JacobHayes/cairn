//! The graph (PRD glossary, Graph; ARCHITECTURE, Engine > Model) in its two forms: the
//! [`RouteFile`], the file document as written (paths and ids, keys optional), and the
//! [`Graph`], the graph document resolved (keys everywhere), which also carries journey
//! state. Route versions and drafts are graphs with empty state.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use std::collections::BTreeMap;

use crate::collections::{
    BoundedVec, HasKey, InsertionCount, Keyed, KindCount, NodeCount, RoleCount,
};
use crate::domain::{Lineage, RouteKind};
use crate::id::{InsertionKey, KindKey, NodeKey, RoleKey, RouteId, Slug};
use crate::node::Node;
use crate::number::VersionNumber;
use crate::refs::{FileRefs, KeyRefs, References, key_absent};
use crate::state::JourneyState;
use crate::text::{Markdown, Title};

/// A role (A6): a named slot, single or multi valued, filled per journey with entities.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields, bound = "")]
#[schemars(bound = "R: References", rename = "Role{R}")]
pub struct Role<R: References> {
    /// The stable key: optional in a file.
    #[serde(skip_serializing_if = "key_absent")]
    pub key: R::RoleKey,
    /// The id, unique in the graph.
    pub id: Slug,
    /// A label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Title>,
    /// Filled by several entities rather than one.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub multi: bool,
}

/// A participation kind a graph declares beyond the built-in `owner` (A7).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields, bound = "")]
#[schemars(bound = "R: References", rename = "ParticipationKind{R}")]
pub struct ParticipationKind<R: References> {
    /// The stable key: optional in a file.
    #[serde(skip_serializing_if = "key_absent")]
    pub key: R::KindKey,
    /// The id, unique in the graph.
    pub id: Slug,
    /// A label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Title>,
    /// Held by several entities rather than one.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub multi: bool,
}

impl HasKey for Role<KeyRefs> {
    type Key = RoleKey;

    fn key(&self) -> &RoleKey {
        &self.key
    }
}

impl HasKey for ParticipationKind<KeyRefs> {
    type Key = KindKey;

    fn key(&self) -> &KindKey {
        &self.key
    }
}

/// The version of the file format a route file is written in. Only version 1 exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct FormatVersion;

impl JsonSchema for FormatVersion {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "FormatVersion".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "const": 1, "description": "The file format version." })
    }
}

impl TryFrom<u32> for FormatVersion {
    type Error = String;

    fn try_from(version: u32) -> Result<Self, String> {
        if version == 1 {
            Ok(FormatVersion)
        } else {
            Err(format!(
                "file format {version} is not supported; this build reads format 1"
            ))
        }
    }
}

impl From<FormatVersion> for u32 {
    fn from(_: FormatVersion) -> u32 {
        1
    }
}

/// The file document (A13, A14; ARCHITECTURE, File format): a route version or draft as
/// written, unresolved. Nodes refer to each other by path and to roles and kinds by id;
/// keys may be left out for import to match by path against the version extended, or mint.
/// The JSON Schema in `schema/` describes this type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteFile {
    /// The file format version.
    pub format: FormatVersion,
    /// The route's id.
    pub route: RouteId,
    /// The route's name.
    pub name: Title,
    /// What the route is for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Markdown>,
    /// Process or segment (A21); a file that leaves it out is a process.
    #[serde(default, skip_serializing_if = "RouteKind::is_process")]
    pub kind: RouteKind,
    /// The published version this one extends, or none for a new route.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<VersionNumber>,
    /// The role that owns nodes no ancestor gives an owner (A6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_owner: Option<Slug>,
    /// The roles.
    #[serde(default, skip_serializing_if = "BoundedVec::is_empty")]
    pub roles: BoundedVec<Role<FileRefs>, RoleCount>,
    /// The participation kinds beyond `owner`.
    #[serde(default, skip_serializing_if = "BoundedVec::is_empty")]
    pub participation_kinds: BoundedVec<ParticipationKind<FileRefs>, KindCount>,
    /// The nodes, each naming its parent by path.
    pub nodes: BoundedVec<Node<FileRefs>, NodeCount>,
}

/// Every key a graph has retired (PRD Invariants: a removed key never comes back as a new
/// object). Tombstones are the upgrade-facing subset of the node keys.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RetiredKeys {
    /// Retired node keys.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub nodes: BTreeSet<NodeKey>,
    /// Retired role keys.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub roles: BTreeSet<RoleKey>,
    /// Retired participation kind keys.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub kinds: BTreeSet<KindKey>,
}

impl RetiredKeys {
    /// True when no key has been retired.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.roles.is_empty() && self.kinds.is_empty()
    }
}

/// The graph document (resolved): nodes, roles, and kinds by key, every reference a key,
/// and, for a journey, its state (ARCHITECTURE, Engine > Model). Collections serialize
/// sorted by key, so a graph has one serialization.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    /// The role that owns nodes no ancestor gives an owner (A6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_owner: Option<RoleKey>,
    /// The roles.
    #[serde(default, skip_serializing_if = "Keyed::is_empty")]
    pub roles: Keyed<Role<KeyRefs>, RoleCount>,
    /// The participation kinds beyond `owner`.
    #[serde(default, skip_serializing_if = "Keyed::is_empty")]
    pub participation_kinds: Keyed<ParticipationKind<KeyRefs>, KindCount>,
    /// The nodes.
    #[serde(default, skip_serializing_if = "Keyed::is_empty")]
    pub nodes: Keyed<Node<KeyRefs>, NodeCount>,
    /// Every key the graph has retired.
    #[serde(default, skip_serializing_if = "RetiredKeys::is_empty")]
    pub retired_keys: RetiredKeys,
    /// The segment insertions the graph holds (B13).
    #[serde(default, skip_serializing_if = "Keyed::is_empty")]
    pub insertions: Keyed<Insertion, InsertionCount>,
    /// Journey state; empty for a route version or draft.
    #[serde(default, skip_serializing_if = "JourneyState::is_empty")]
    pub state: JourneyState,
}

/// One placement of a segment version in a graph (PRD glossary, Insertion; B13). Its local
/// edits are not stored: they are its members' differences from the version it is on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Insertion {
    /// The key, minted by the client like any new key; unique in the graph.
    pub key: InsertionKey,
    /// The segment and the version the insertion is on.
    pub segment: Lineage,
    /// Where it was inserted: the parent the root went under, or none for the top level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<NodeKey>,
    /// Each segment role, mapped to the graph role it became.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "crate::serde_util::unique_map_per_role"
    )]
    pub roles: BTreeMap<RoleKey, RoleKey>,
    /// Each segment participation kind beyond `owner`, likewise.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "crate::serde_util::unique_map_per_kind"
    )]
    pub kinds: BTreeMap<KindKey, KindKey>,
    /// Its members: each graph node it copied that the graph still holds, with that node's
    /// key in the segment.
    #[serde(deserialize_with = "crate::serde_util::unique_map_per_node")]
    pub nodes: BTreeMap<NodeKey, NodeKey>,
}

impl HasKey for Insertion {
    type Key = InsertionKey;

    fn key(&self) -> &InsertionKey {
        &self.key
    }
}

/// An explicit `requires` edge (A3), identified by its two ends: `node` requires `requires`.
/// The graph document holds edges on the dependent node's `requires` list; changes and
/// removals name them this way.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    /// The dependent node.
    pub node: NodeKey,
    /// The node it requires.
    pub requires: NodeKey,
}
