//! Serde helpers shared by the string-backed newtypes: each one serializes as its text and
//! parses through `FromStr`, so a value that breaks its rules (or a limit) fails to
//! deserialize with the reason.

/// Implements `Display`, `Serialize`, `Deserialize`, and `JsonSchema` for a type that has
/// `as_str()` and `FromStr`. The JSON Schema is a string, with `$pattern` and the length
/// bounds of `$max_bytes` (a [`crate::limits::Limit`]) when given.
macro_rules! string_serde {
    ($type:ty, $schema_name:expr, $description:expr) => {
        $crate::serde_util::string_serde!($type, $schema_name, $description, None::<&str>, None);
    };
    ($type:ty, $schema_name:expr, $description:expr, $pattern:expr) => {
        $crate::serde_util::string_serde!($type, $schema_name, $description, $pattern, None);
    };
    ($type:ty, $schema_name:expr, $description:expr, $pattern:expr, $max_bytes:expr) => {
        impl ::std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl ::serde::Serialize for $type {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $type {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text = <String as ::serde::Deserialize>::deserialize(deserializer)?;
                text.parse().map_err(::serde::de::Error::custom)
            }
        }

        impl ::schemars::JsonSchema for $type {
            fn schema_name() -> ::std::borrow::Cow<'static, str> {
                $schema_name.into()
            }

            fn json_schema(_: &mut ::schemars::SchemaGenerator) -> ::schemars::Schema {
                let mut schema = ::schemars::json_schema!({
                    "type": "string",
                    "description": $description,
                });
                if let Some(pattern) = $pattern {
                    schema.insert("pattern".into(), pattern.into());
                }
                // JSON Schema counts characters and the limits count bytes, so this bound is
                // necessary rather than sufficient for text beyond ASCII.
                let max_bytes: Option<$crate::limits::Limit> = $max_bytes;
                if let Some(limit) = max_bytes {
                    schema.insert("minLength".into(), 1.into());
                    schema.insert("maxLength".into(), limit.max().into());
                }
                schema
            }
        }
    };
}

pub(crate) use string_serde;

/// `skip_serializing_if` for flags that default to false. Serde passes fields by reference.
#[allow(clippy::trivially_copy_pass_by_ref)]
pub(crate) fn is_false(flag: &bool) -> bool {
    !flag
}

/// `deserialize_with` for an optional field that, when present, must hold a value: an
/// explicit `null` is an error rather than an absent field, so a field a node kind may not
/// carry is rejected however it is written (A1a).
pub(crate) fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// A JSON Schema `transform` requiring exactly one of `fields`: what the `TryFrom` checks of
/// the flat attachment and date-rule forms enforce, so the schema says it too.
pub(crate) fn exactly_one_of(schema: &mut schemars::Schema, fields: &[&str]) {
    let options: Vec<serde_json::Value> = fields
        .iter()
        .map(|field| serde_json::json!({ "required": [field] }))
        .collect();
    schema.insert("oneOf".to_owned(), serde_json::Value::Array(options));
}

/// Reads a map entry by entry and rejects a key written twice. JSON allows repeated keys and
/// serde's own map keeps the last, while YAML's reader rejects them, so without this the two
/// formats would read one document differently. With `limit`, the entry count is checked as
/// entries arrive, so an oversized map fails before it is all read.
pub(crate) fn unique_entries<'de, D, K, V>(
    deserializer: D,
    expecting: &'static str,
    limit: Option<crate::limits::Limit>,
) -> Result<std::collections::BTreeMap<K, V>, D::Error>
where
    D: serde::Deserializer<'de>,
    K: serde::Deserialize<'de> + Ord + std::fmt::Display,
    V: serde::Deserialize<'de>,
{
    struct Entries<K, V> {
        expecting: &'static str,
        limit: Option<crate::limits::Limit>,
        types: std::marker::PhantomData<(K, V)>,
    }

    impl<'de, K, V> serde::de::Visitor<'de> for Entries<K, V>
    where
        K: serde::Deserialize<'de> + Ord + std::fmt::Display,
        V: serde::Deserialize<'de>,
    {
        type Value = std::collections::BTreeMap<K, V>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str(self.expecting)
        }

        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut access: A,
        ) -> Result<Self::Value, A::Error> {
            let mut map = std::collections::BTreeMap::new();
            while let Some((key, value)) = access.next_entry::<K, V>()? {
                if map.contains_key(&key) {
                    return Err(serde::de::Error::custom(format!("{key} appears twice")));
                }
                map.insert(key, value);
                if let Some(limit) = self.limit {
                    limit.check(map.len()).map_err(serde::de::Error::custom)?;
                }
            }
            Ok(map)
        }
    }

    deserializer.deserialize_map(Entries {
        expecting,
        limit,
        types: std::marker::PhantomData,
    })
}

/// `deserialize_with` for a keyed map field: [`unique_entries`] without a limit, so a key
/// written twice is an error in JSON as it is in YAML.
pub(crate) fn unique_map<'de, D, K, V>(
    deserializer: D,
) -> Result<std::collections::BTreeMap<K, V>, D::Error>
where
    D: serde::Deserializer<'de>,
    K: serde::Deserialize<'de> + Ord + std::fmt::Display,
    V: serde::Deserialize<'de>,
{
    unique_entries(deserializer, "a map with each key once", None)
}

/// `deserialize_with` for a map with one entry per node: [`unique_entries`] held to
/// `node_count_max`, since a graph has no more nodes to key it by.
pub(crate) fn unique_map_per_node<'de, D, K, V>(
    deserializer: D,
) -> Result<std::collections::BTreeMap<K, V>, D::Error>
where
    D: serde::Deserializer<'de>,
    K: serde::Deserialize<'de> + Ord + std::fmt::Display,
    V: serde::Deserialize<'de>,
{
    unique_entries(
        deserializer,
        "a map from node to its entry, each node once",
        Some(crate::limits::Limit::NodeCount),
    )
}

/// `deserialize_with` for a map with one entry per role: [`unique_entries`] held to
/// `role_count_max`, since a graph has no more roles to key it by.
pub(crate) fn unique_map_per_role<'de, D, K, V>(
    deserializer: D,
) -> Result<std::collections::BTreeMap<K, V>, D::Error>
where
    D: serde::Deserializer<'de>,
    K: serde::Deserialize<'de> + Ord + std::fmt::Display,
    V: serde::Deserialize<'de>,
{
    unique_entries(
        deserializer,
        "a map from role to its entry, each role once",
        Some(crate::limits::Limit::RoleCount),
    )
}
