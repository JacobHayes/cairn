//! Collections that hold their limit (PRACTICES, Explicit limits) and their order
//! (PRACTICES, Deterministic by construction): a bounded list keeps the order it was given,
//! a bounded set and a keyed map keep sorted order, and every one of them is checked when
//! it is deserialized or built.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::marker::PhantomData;

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::limits::{Limit, LimitExceeded};

/// A type-level name for one of the [`Limit`]s.
pub trait LimitOf: Send + Sync + 'static {
    /// The limit.
    const LIMIT: Limit;
}

macro_rules! limit_markers {
    ($($(#[$doc:meta])* $marker:ident => $limit:ident),* $(,)?) => {$(
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $marker {}

        impl LimitOf for $marker {
            const LIMIT: Limit = Limit::$limit;
        }
    )*};
}

limit_markers! {
    /// `node_count_max`.
    NodeCount => NodeCount,
    /// `edge_count_per_node_max`.
    EdgeCountPerNode => EdgeCountPerNode,
    /// `mutation_count_per_patch_max`.
    MutationCountPerPatch => MutationCountPerPatch,
    /// `role_count_max`.
    RoleCount => RoleCount,
    /// `kind_count_max`.
    KindCount => KindCount,
    /// `choice_count_per_decision_max`.
    ChoiceCountPerDecision => ChoiceCountPerDecision,
    /// `entity_count_per_fill_max`.
    EntityCountPerFill => EntityCountPerFill,
    /// A collection the limits table does not name, bounded by the serialized graph cap
    /// (`graph_bytes_max`) and, for any document, the request cap (`request_bytes_max`), which
    /// every parse checks first (DECISIONS.md).
    ByDocumentSize => GraphBytes,
    /// `chain_count_per_rejection_max`.
    ChainCountPerRejection => ChainCountPerRejection,
    /// `explanation_entry_count_max`.
    ExplanationEntryCount => ExplanationEntryCount,
}

/// Why a collection was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CollectionError {
    /// More items than its limit.
    TooMany(LimitExceeded),
    /// The same item, or key, twice.
    Duplicate(String),
    /// No items where at least one is required.
    Empty,
}

impl fmt::Display for CollectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CollectionError::TooMany(exceeded) => write!(formatter, "too many items: {exceeded}"),
            CollectionError::Duplicate(item) => write!(formatter, "{item} appears twice"),
            CollectionError::Empty => formatter.write_str("at least one item is required"),
        }
    }
}

impl std::error::Error for CollectionError {}

/// A list in the order given, at most `L` long.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundedVec<T, L: LimitOf>(Vec<T>, PhantomData<L>);

impl<T, L: LimitOf> BoundedVec<T, L> {
    /// Checks the length.
    ///
    /// # Errors
    ///
    /// When `items` is past the limit.
    pub fn new(items: Vec<T>) -> Result<Self, CollectionError> {
        L::LIMIT
            .check(items.len())
            .map_err(CollectionError::TooMany)?;
        Ok(Self(items, PhantomData))
    }

    /// The items.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// The items, to edit in place; the count, and so the bound, cannot change.
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.0
    }

    /// The number of items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// True when there are no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The items, by value.
    #[must_use]
    pub fn into_vec(self) -> Vec<T> {
        self.0
    }
}

impl<T, L: LimitOf> Default for BoundedVec<T, L> {
    fn default() -> Self {
        Self(Vec::new(), PhantomData)
    }
}

impl<T: Serialize, L: LimitOf> Serialize for BoundedVec<T, L> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de, T: Deserialize<'de>, L: LimitOf> Deserialize<'de> for BoundedVec<T, L> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(Vec::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl<T: JsonSchema, L: LimitOf> JsonSchema for BoundedVec<T, L> {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        format!("BoundedList_{}", T::schema_name()).into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "array",
            "items": generator.subschema_for::<T>(),
            "maxItems": L::LIMIT.max(),
        })
    }

    fn inline_schema() -> bool {
        true
    }
}

/// A sorted set with no duplicates, at most `L` items. Deserializing rejects a duplicate
/// rather than dropping it, so what parses is what was written.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundedSet<T: Ord, L: LimitOf>(BTreeSet<T>, PhantomData<L>);

impl<T: Ord + fmt::Debug, L: LimitOf> BoundedSet<T, L> {
    /// Collects `items`, rejecting duplicates and checking the count.
    ///
    /// # Errors
    ///
    /// When an item repeats or the count is past the limit.
    pub fn new(items: impl IntoIterator<Item = T>) -> Result<Self, CollectionError> {
        let mut set = BTreeSet::new();
        for item in items {
            if set.contains(&item) {
                return Err(CollectionError::Duplicate(format!("{item:?}")));
            }
            set.insert(item);
            L::LIMIT
                .check(set.len())
                .map_err(CollectionError::TooMany)?;
        }
        Ok(Self(set, PhantomData))
    }

    /// The items, sorted.
    #[must_use]
    pub fn as_set(&self) -> &BTreeSet<T> {
        &self.0
    }

    /// The number of items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// True when there are no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterates in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter()
    }
}

impl<T: Ord, L: LimitOf> Default for BoundedSet<T, L> {
    fn default() -> Self {
        Self(BTreeSet::new(), PhantomData)
    }
}

impl<T: Ord + Serialize, L: LimitOf> Serialize for BoundedSet<T, L> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(&self.0)
    }
}

impl<'de, T: Ord + fmt::Debug + Deserialize<'de>, L: LimitOf> Deserialize<'de>
    for BoundedSet<T, L>
{
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(Vec::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl<T: Ord + JsonSchema, L: LimitOf> JsonSchema for BoundedSet<T, L> {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        format!("BoundedSet_{}", T::schema_name()).into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "array",
            "items": generator.subschema_for::<T>(),
            "uniqueItems": true,
            "maxItems": L::LIMIT.max(),
        })
    }

    fn inline_schema() -> bool {
        true
    }
}

/// A value that carries its own key, so a [`Keyed`] map can serialize as a plain list.
pub trait HasKey {
    /// The key type.
    type Key: Ord + Clone + fmt::Debug;

    /// The value's key.
    fn key(&self) -> &Self::Key;
}

/// A map from each value's own key to the value, at most `L` entries. Serializes as a list
/// sorted by key, and deserializing rejects a key that appears twice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keyed<V: HasKey, L: LimitOf>(BTreeMap<V::Key, V>, PhantomData<L>);

impl<V: HasKey, L: LimitOf> Keyed<V, L> {
    /// Collects `values` by key.
    ///
    /// # Errors
    ///
    /// When two values share a key or the count is past the limit.
    pub fn new(values: impl IntoIterator<Item = V>) -> Result<Self, CollectionError> {
        let mut map = BTreeMap::new();
        for value in values {
            let key = value.key().clone();
            if map.contains_key(&key) {
                return Err(CollectionError::Duplicate(format!("key {key:?}")));
            }
            map.insert(key, value);
            L::LIMIT
                .check(map.len())
                .map_err(CollectionError::TooMany)?;
        }
        Ok(Self(map, PhantomData))
    }

    /// The map.
    #[must_use]
    pub fn as_map(&self) -> &BTreeMap<V::Key, V> {
        &self.0
    }

    /// The value with `key`.
    #[must_use]
    pub fn get(&self, key: &V::Key) -> Option<&V> {
        self.0.get(key)
    }

    /// The values, sorted by key.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.0.values()
    }

    /// The number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// True when there are no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Inserts or replaces the value under its own key.
    ///
    /// # Errors
    ///
    /// When a new key would take the map past its limit; the map is unchanged.
    pub fn put(&mut self, value: V) -> Result<Option<V>, CollectionError> {
        if !self.0.contains_key(value.key()) {
            L::LIMIT
                .check(self.0.len() + 1)
                .map_err(CollectionError::TooMany)?;
        }
        Ok(self.0.insert(value.key().clone(), value))
    }

    /// Removes and returns the value under `key`.
    pub fn remove(&mut self, key: &V::Key) -> Option<V> {
        self.0.remove(key)
    }
}

impl<V: HasKey, L: LimitOf> Default for Keyed<V, L> {
    fn default() -> Self {
        Self(BTreeMap::new(), PhantomData)
    }
}

impl<V: HasKey + Serialize, L: LimitOf> Serialize for Keyed<V, L> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.0.values())
    }
}

impl<'de, V: HasKey + DeserializeOwned, L: LimitOf> Deserialize<'de> for Keyed<V, L> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(Vec::<V>::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl<V: HasKey + JsonSchema, L: LimitOf> JsonSchema for Keyed<V, L> {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        format!("Keyed_{}", V::schema_name()).into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "array",
            "items": generator.subschema_for::<V>(),
            "maxItems": L::LIMIT.max(),
        })
    }

    fn inline_schema() -> bool {
        true
    }
}

/// One item, or several: serializes a single item bare and several as a list, and reads
/// either (A8: a date rule names one source or several).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OneOrMany<T: Ord, L: LimitOf>(BoundedSet<T, L>);

impl<T: Ord + fmt::Debug, L: LimitOf> OneOrMany<T, L> {
    /// Collects at least one item.
    ///
    /// # Errors
    ///
    /// When there are none, one repeats, or there are too many.
    pub fn new(items: impl IntoIterator<Item = T>) -> Result<Self, CollectionError> {
        let set = BoundedSet::new(items)?;
        if set.is_empty() {
            return Err(CollectionError::Empty);
        }
        Ok(Self(set))
    }

    /// The items, sorted.
    #[must_use]
    pub fn as_set(&self) -> &BTreeSet<T> {
        self.0.as_set()
    }
}

impl<T: Ord + Serialize, L: LimitOf> Serialize for OneOrMany<T, L> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut items = self.0.0.iter();
        match (items.next(), items.next()) {
            (Some(only), None) => only.serialize(serializer),
            _ => self.0.serialize(serializer),
        }
    }
}

impl<'de, T: Ord + fmt::Debug + DeserializeOwned, L: LimitOf> Deserialize<'de> for OneOrMany<T, L> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged, bound = "T: DeserializeOwned")]
        enum Form<T> {
            Many(Vec<T>),
            One(T),
        }
        let items = match Form::<T>::deserialize(deserializer)? {
            Form::Many(items) => items,
            Form::One(item) => vec![item],
        };
        Self::new(items).map_err(serde::de::Error::custom)
    }
}

impl<T: Ord + JsonSchema, L: LimitOf> JsonSchema for OneOrMany<T, L> {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        format!("OneOrMany_{}", T::schema_name()).into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let item = generator.subschema_for::<T>();
        schemars::json_schema!({
            "anyOf": [
                item,
                { "type": "array", "items": item, "minItems": 1, "uniqueItems": true, "maxItems": L::LIMIT.max() },
            ],
        })
    }

    fn inline_schema() -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Choices = BoundedVec<u32, ChoiceCountPerDecision>;
    type Entities = BoundedSet<u32, EntityCountPerFill>;

    #[test]
    fn bounded_list_at_and_past_its_limit() {
        let at: Vec<u32> = (0..32).collect();
        assert!(serde_json::from_value::<Choices>(serde_json::json!(at)).is_ok());
        let past: Vec<u32> = (0..33).collect();
        let error = serde_json::from_value::<Choices>(serde_json::json!(past)).unwrap_err();
        assert!(
            error
                .to_string()
                .contains(Limit::ChoiceCountPerDecision.name())
        );
    }

    #[test]
    fn bounded_set_at_and_past_its_limit() {
        let at: Vec<u32> = (0..100).collect();
        assert!(serde_json::from_value::<Entities>(serde_json::json!(at)).is_ok());
        let past: Vec<u32> = (0..101).collect();
        let error = serde_json::from_value::<Entities>(serde_json::json!(past)).unwrap_err();
        assert!(error.to_string().contains(Limit::EntityCountPerFill.name()));
    }

    #[test]
    fn bounded_set_rejects_duplicates_and_sorts() {
        assert!(serde_json::from_str::<Entities>("[2, 1, 2]").is_err());
        let set: Entities = serde_json::from_str("[3, 1, 2]").unwrap();
        assert_eq!(serde_json::to_string(&set).unwrap(), "[1,2,3]");
    }

    #[test]
    fn one_or_many_writes_one_bare() {
        type Sources = OneOrMany<u32, EdgeCountPerNode>;
        let one: Sources = serde_json::from_str("7").unwrap();
        assert_eq!(serde_json::to_string(&one).unwrap(), "7");
        let many: Sources = serde_json::from_str("[9, 7]").unwrap();
        assert_eq!(serde_json::to_string(&many).unwrap(), "[7,9]");
        assert!(serde_json::from_str::<Sources>("[]").is_err());
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Item {
        key: u32,
        name: String,
    }

    impl HasKey for Item {
        type Key = u32;

        fn key(&self) -> &u32 {
            &self.key
        }
    }

    #[test]
    fn keyed_rejects_a_repeated_key_and_holds_its_limit() {
        let text = r#"[{"key":2,"name":"b"},{"key":1,"name":"a"}]"#;
        let map: Keyed<Item, RoleCount> = serde_json::from_str(text).unwrap();
        assert_eq!(
            serde_json::to_string(&map).unwrap(),
            r#"[{"key":1,"name":"a"},{"key":2,"name":"b"}]"#
        );
        let repeated = r#"[{"key":1,"name":"a"},{"key":1,"name":"b"}]"#;
        assert!(serde_json::from_str::<Keyed<Item, RoleCount>>(repeated).is_err());
        let mut full: Keyed<Item, RoleCount> = Keyed::new((0..32).map(|key| Item {
            key,
            name: String::new(),
        }))
        .unwrap();
        let past = full
            .put(Item {
                key: 99,
                name: String::new(),
            })
            .unwrap_err();
        assert!(matches!(past, CollectionError::TooMany(e) if e.limit == Limit::RoleCount));
        assert!(
            full.put(Item {
                key: 0,
                name: "replaced".into()
            })
            .is_ok()
        );
    }
}
