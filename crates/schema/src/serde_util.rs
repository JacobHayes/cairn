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
