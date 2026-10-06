//! The two forms of one document schema (ARCHITECTURE, File format): the file form, as
//! written, refers to nodes by path and to roles and kinds by id, and may leave keys out
//! for import to match or mint; the graph form, resolved, refers to everything by key and
//! always carries keys. The graph types are generic over [`References`], so the file
//! document and the graph document are two types with one shape (PRD, Identity and
//! references: references are stored by key; paths and ids are the serialization form).

use std::fmt;
use std::hash::Hash;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::id::{AttachmentKey, KindKey, NodeKey, Path, RoleKey, Slug};

/// A value one document form uses to refer to something: text in both formats.
pub trait Reference:
    Clone
    + fmt::Debug
    + fmt::Display
    + FromStr<Err: fmt::Display>
    + Ord
    + Hash
    + Serialize
    + DeserializeOwned
    + JsonSchema
    + Send
    + Sync
    + 'static
{
}

impl<T> Reference for T where
    T: Clone
        + fmt::Debug
        + fmt::Display
        + FromStr<Err: fmt::Display>
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned
        + JsonSchema
        + Send
        + Sync
        + 'static
{
}

/// Where an object's own key goes: optional in a file (import matches or mints a missing
/// one), required in a graph.
pub trait KeySlot:
    Clone + fmt::Debug + Ord + Hash + Serialize + DeserializeOwned + JsonSchema + Send + Sync + 'static
{
    /// The key, when there is one.
    type Key;

    /// True when the key is left out, so serialization omits the field.
    fn is_absent(&self) -> bool;
}

impl<K> KeySlot for Option<K>
where
    K: Clone
        + fmt::Debug
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned
        + JsonSchema
        + Send
        + Sync
        + 'static,
{
    type Key = K;

    fn is_absent(&self) -> bool {
        self.is_none()
    }
}

macro_rules! required_key_slot {
    ($($key:ty),*) => {$(
        impl KeySlot for $key {
            type Key = $key;

            fn is_absent(&self) -> bool {
                false
            }
        }
    )*};
}

required_key_slot!(NodeKey, RoleKey, KindKey, AttachmentKey);

/// The reference and key types of one document form.
pub trait References:
    Clone + fmt::Debug + PartialEq + Eq + PartialOrd + Ord + Hash + JsonSchema + Send + Sync + 'static
{
    /// A reference to a node: a path in a file, a key in a graph.
    type Node: Reference;
    /// A reference to a role: an id in a file, a key in a graph.
    type Role: Reference;
    /// A reference to a participation kind: an id in a file, a key in a graph.
    type Kind: Reference;
    /// A node's own key.
    type NodeKey: KeySlot<Key = NodeKey>;
    /// A role's own key.
    type RoleKey: KeySlot<Key = RoleKey>;
    /// A participation kind's own key.
    type KindKey: KeySlot<Key = KindKey>;
    /// A resource's own key.
    type AttachmentKey: KeySlot<Key = AttachmentKey>;
    /// The reference to the built-in `owner` kind.
    fn owner_kind() -> Self::Kind;
}

/// The file form (unresolved, as written): paths and ids, optional keys.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileRefs {}

/// The graph form (resolved): keys everywhere.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KeyRefs {}

impl References for FileRefs {
    type Node = Path;
    type Role = Slug;
    type Kind = Slug;
    type NodeKey = Option<NodeKey>;
    type RoleKey = Option<RoleKey>;
    type KindKey = Option<KindKey>;
    type AttachmentKey = Option<AttachmentKey>;
    fn owner_kind() -> Slug {
        crate::id::owner_kind_id()
    }
}

impl References for KeyRefs {
    type Node = NodeKey;
    type Role = RoleKey;
    type Kind = KindKey;
    type NodeKey = NodeKey;
    type RoleKey = RoleKey;
    type KindKey = KindKey;
    type AttachmentKey = AttachmentKey;
    fn owner_kind() -> KindKey {
        KindKey::owner()
    }
}

/// `skip_serializing_if` for a key slot.
pub(crate) fn key_absent<S: KeySlot>(slot: &S) -> bool {
    slot.is_absent()
}

// The form's schema name is the suffix generic types carry in the JSON Schema
// (`#[schemars(rename = "Node{R}")]`): nothing for the file form, which is the one the
// published schema describes, and `Resolved` for the graph form.
impl JsonSchema for FileRefs {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!(false)
    }
}

impl JsonSchema for KeyRefs {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Resolved".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!(false)
    }
}
