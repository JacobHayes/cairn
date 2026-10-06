//! Bounded numbers (PRACTICES, Fixed-width integers; Explicit limits): revisions, version
//! numbers, day counts, and weights, each a `u32` newtype checked when parsed.

use std::fmt;

use crate::limits::{Limit, LimitExceeded};

/// Declares a `u32` newtype that serializes as a number and checks `$check` on parse.
macro_rules! bounded_number {
    ($(#[$doc:meta])* $name:ident, $minimum:expr, $maximum:expr, $check:expr) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u32);

        impl $name {
            /// The value.
            #[must_use]
            pub const fn get(self) -> u32 {
                self.0
            }
        }

        impl TryFrom<u32> for $name {
            type Error = NumberError;

            fn try_from(value: u32) -> Result<Self, NumberError> {
                let check: fn(u32) -> Result<(), NumberError> = $check;
                check(value)?;
                Ok(Self(value))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_u32(self.0)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = u32::deserialize(deserializer)?;
                Self::try_from(value).map_err(serde::de::Error::custom)
            }
        }

        impl schemars::JsonSchema for $name {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                stringify!($name).into()
            }

            fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
                let minimum: u32 = $minimum;
                let maximum: Option<u32> = $maximum;
                let mut schema = schemars::json_schema!({ "type": "integer", "minimum": minimum });
                if let Some(maximum) = maximum {
                    schema.insert("maximum".into(), maximum.into());
                }
                schema
            }
        }
    };
}

/// Why a number was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberError {
    /// Past its limit.
    TooLarge(LimitExceeded),
    /// Zero where counting starts at one.
    Zero,
}

impl fmt::Display for NumberError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NumberError::TooLarge(exceeded) => write!(formatter, "number is too large: {exceeded}"),
            NumberError::Zero => formatter.write_str("zero, where counting starts at 1"),
        }
    }
}

impl std::error::Error for NumberError {}

fn within(limit: Limit) -> impl Fn(u32) -> Result<(), NumberError> {
    move |value| {
        limit
            .check(usize::try_from(value).unwrap_or(usize::MAX))
            .map_err(NumberError::TooLarge)
    }
}

bounded_number!(
    /// A revision of a patch domain or proposal (H5). 0 is "does not exist yet": a patch
    /// with base revision 0 creates its target, which it leaves at revision 1 (A17).
    Revision, 0, None, |_| Ok(())
);
bounded_number!(
    /// A route version's number, counting from 1 (A11).
    VersionNumber, 1, None, |value| if value == 0 { Err(NumberError::Zero) } else { Ok(()) }
);
bounded_number!(
    /// Calendar days in a date offset or estimate, at most `offset_days_max` (A8, A9).
    Days, 0, Some(crate::limits::OFFSET_DAYS_MAX), |value| within(Limit::OffsetDays)(value)
);
bounded_number!(
    /// A node's authored weight, at most `weight_max` (A9).
    Weight, 0, Some(crate::limits::WEIGHT_MAX), |value| within(Limit::Weight)(value)
);

/// A signed day count within `offset_days_max` either way: a pin shift (F5 `shift <delta>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SignedDays(i32);

impl SignedDays {
    /// The value.
    #[must_use]
    pub const fn get(self) -> i32 {
        self.0
    }
}

impl TryFrom<i32> for SignedDays {
    type Error = NumberError;

    fn try_from(value: i32) -> Result<Self, NumberError> {
        within(Limit::OffsetDays)(value.unsigned_abs())?;
        Ok(Self(value))
    }
}

impl fmt::Display for SignedDays {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl serde::Serialize for SignedDays {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i32(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for SignedDays {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = i32::deserialize(deserializer)?;
        Self::try_from(value).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for SignedDays {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "SignedDays".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let maximum = crate::limits::OFFSET_DAYS_MAX;
        schemars::json_schema!({ "type": "integer", "minimum": -i64::from(maximum), "maximum": maximum })
    }
}

impl Revision {
    /// The revision of a target that does not exist yet.
    pub const NONE: Revision = Revision(0);

    /// The revision after this one.
    ///
    /// # Panics
    ///
    /// At `u32::MAX`, which no domain reaches.
    #[must_use]
    pub fn next(self) -> Revision {
        assert!(self.0 < u32::MAX, "revision overflow");
        Revision(self.0 + 1)
    }
}

impl Default for Revision {
    fn default() -> Self {
        Revision::NONE
    }
}

impl VersionNumber {
    /// The first version of a route.
    pub const FIRST: VersionNumber = VersionNumber(1);

    /// The version after this one.
    ///
    /// # Panics
    ///
    /// At `u32::MAX`, which no domain reaches.
    #[must_use]
    pub fn next(self) -> VersionNumber {
        assert!(self.0 < u32::MAX, "version overflow");
        VersionNumber(self.0 + 1)
    }
}

impl Weight {
    /// The default weight of every kind but `group` (A9).
    pub const DEFAULT: Weight = Weight(1);
    /// The default weight of a group (A9).
    pub const GROUP_DEFAULT: Weight = Weight(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_and_weight_at_and_past_their_limits() {
        let cases = [
            (
                Limit::OffsetDays,
                serde_json::from_str::<Days>("365").map(|_| ()),
                serde_json::from_str::<Days>("366").map(|_| ()),
            ),
            (
                Limit::Weight,
                serde_json::from_str::<Weight>("1000").map(|_| ()),
                serde_json::from_str::<Weight>("1001").map(|_| ()),
            ),
        ];
        for (limit, at, past) in cases {
            assert!(at.is_ok(), "{limit}");
            assert!(past.unwrap_err().to_string().contains(limit.name()));
        }
    }

    #[test]
    fn negative_days_and_weights_do_not_parse() {
        assert!(serde_json::from_str::<Days>("-1").is_err());
        assert!(serde_json::from_str::<Weight>("-1").is_err());
        assert!(serde_json::from_str::<Weight>("1.5").is_err());
    }

    #[test]
    fn signed_days_at_and_past_their_limit_either_way() {
        for (value, ok) in [(365, true), (-365, true), (366, false), (-366, false)] {
            assert_eq!(SignedDays::try_from(value).is_ok(), ok, "{value}");
        }
    }

    #[test]
    fn versions_count_from_one() {
        assert_eq!(VersionNumber::try_from(0), Err(NumberError::Zero));
        assert_eq!(VersionNumber::FIRST.next().get(), 2);
        assert_eq!(Revision::NONE.next().get(), 1);
    }
}
