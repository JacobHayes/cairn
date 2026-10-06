//! Identifiers (PRD, Identity and references; ARCHITECTURE, Engine > Model): id slugs and
//! paths, the readable serialization and display form, and keys, the stable opaque
//! identity every reference is stored by. Each key type has its own fixed prefix, so a key
//! of one type never parses as another, and none of them is an id: a key and an id cannot
//! be swapped in a reference, at compile time or in a document.

use std::fmt;
use std::str::FromStr;

use crate::limits::{CONTAINMENT_DEPTH_MAX, Limit, LimitExceeded};
use crate::serde_util::string_serde;

/// Why an identifier was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdError {
    /// Past its byte or segment limit.
    TooLong(LimitExceeded),
    /// Not a slug: lowercase ASCII letters, digits, `_`, and `-`, starting with a letter or
    /// digit.
    NotSlug(String),
    /// A key without its type's prefix, or with a malformed body.
    NotKey {
        /// The prefix the key type requires.
        prefix: &'static str,
        /// The text that was given.
        text: String,
    },
    /// A path with an empty segment.
    EmptySegment(String),
}

impl fmt::Display for IdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdError::TooLong(exceeded) => write!(formatter, "identifier is too long: {exceeded}"),
            IdError::NotSlug(text) => write!(
                formatter,
                "{text:?} is not an id slug (lowercase letters, digits, '_' and '-', starting \
                 with a letter or digit)"
            ),
            IdError::NotKey { prefix, text } => write!(
                formatter,
                "{text:?} is not a key of this type: expected {prefix:?} followed by a slug"
            ),
            IdError::EmptySegment(text) => write!(formatter, "path {text:?} has an empty segment"),
        }
    }
}

impl std::error::Error for IdError {}

const SLUG_PATTERN: &str = "^[a-z0-9][a-z0-9_-]*$";

fn is_slug(text: &str) -> bool {
    text.starts_with(|first: char| first.is_ascii_lowercase() || first.is_ascii_digit())
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// An id: a slug unique among its siblings (nodes) or within its graph (roles, kinds).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Slug(String);

impl Slug {
    /// The slug.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Slug {
    type Err = IdError;

    fn from_str(text: &str) -> Result<Self, IdError> {
        Limit::IdBytes.check(text.len()).map_err(IdError::TooLong)?;
        if !is_slug(text) {
            return Err(IdError::NotSlug(text.to_owned()));
        }
        Ok(Self(text.to_owned()))
    }
}

string_serde!(
    Slug,
    "Slug",
    "An id: lowercase letters, digits, '_' and '-', at most id_bytes_max (64) bytes.",
    Some(SLUG_PATTERN),
    Some(Limit::IdBytes)
);

/// A node's path: the slash-joined ids from the root, at most `containment_depth_max`
/// segments. Paths order segment by segment, so sorting paths walks the tree depth first
/// with siblings by id.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Path(Vec<Slug>);

impl Path {
    /// A root node's path.
    #[must_use]
    pub fn root(id: Slug) -> Self {
        Self(vec![id])
    }

    /// The path of a child with `id` under this path.
    ///
    /// # Errors
    ///
    /// When the child would be deeper than `containment_depth_max`.
    pub fn child(&self, id: Slug) -> Result<Self, IdError> {
        let mut segments = self.0.clone();
        segments.push(id);
        Limit::ContainmentDepth
            .check(segments.len())
            .map_err(IdError::TooLong)?;
        Ok(Self(segments))
    }

    /// The node's own id: the last segment.
    #[must_use]
    pub fn id(&self) -> &Slug {
        match self.0.last() {
            Some(id) => id,
            None => unreachable!("a path has at least one segment"),
        }
    }

    /// The parent's path, or `None` for a root.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        match self.0.split_last() {
            Some((_, [])) | None => None,
            Some((_, parent)) => Some(Self(parent.to_vec())),
        }
    }

    /// The ids from the root.
    #[must_use]
    pub fn segments(&self) -> &[Slug] {
        &self.0
    }

    /// The number of segments: 1 for a root.
    ///
    /// # Panics
    ///
    /// Never for a parsed path: parsing holds the depth to `containment_depth_max`.
    #[must_use]
    pub fn depth(&self) -> u32 {
        let depth = u32::try_from(self.0.len()).unwrap_or(u32::MAX);
        assert!((1..=CONTAINMENT_DEPTH_MAX).contains(&depth));
        depth
    }
}

impl FromStr for Path {
    type Err = IdError;

    fn from_str(text: &str) -> Result<Self, IdError> {
        let mut segments = Vec::new();
        for segment in text.split('/') {
            if segment.is_empty() {
                return Err(IdError::EmptySegment(text.to_owned()));
            }
            segments.push(segment.parse()?);
            Limit::ContainmentDepth
                .check(segments.len())
                .map_err(IdError::TooLong)?;
        }
        Ok(Self(segments))
    }
}

impl fmt::Display for Path {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, segment) in self.0.iter().enumerate() {
            if index > 0 {
                formatter.write_str("/")?;
            }
            formatter.write_str(segment.as_str())?;
        }
        Ok(())
    }
}

impl serde::Serialize for Path {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for Path {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for Path {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Path".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "description": "A node path: slash-joined ids from the root, at most containment_depth_max (16) segments.",
            // Each segment within id_bytes_max, at most containment_depth_max segments.
            "pattern": format!(
                "^{segment}(/{segment}){{0,{more}}}$",
                segment = format!("[a-z0-9][a-z0-9_-]{{0,{}}}", crate::limits::ID_BYTES_MAX - 1),
                more = CONTAINMENT_DEPTH_MAX - 1,
            ),
            "maxLength": CONTAINMENT_DEPTH_MAX * (crate::limits::ID_BYTES_MAX + 1) - 1,
        })
    }
}

/// A key or id with a fixed prefix: the prefix, then a slug-shaped body, at most
/// `id_bytes_max` bytes in all.
pub trait Prefixed: FromStr<Err = IdError> + fmt::Display + Sized {
    /// The prefix every value of this type starts with.
    const PREFIX: &'static str;

    /// Builds a value from a body minted by a [`crate::KeyAllocator`].
    ///
    /// # Errors
    ///
    /// When the body is not a slug or the whole is too long.
    fn from_body(body: &str) -> Result<Self, IdError> {
        format!("{}{body}", Self::PREFIX).parse()
    }
}

fn parse_prefixed(text: &str, prefix: &'static str) -> Result<(), IdError> {
    Limit::IdBytes.check(text.len()).map_err(IdError::TooLong)?;
    match text.strip_prefix(prefix) {
        Some(body) if is_slug(body) => Ok(()),
        _ => Err(IdError::NotKey {
            prefix,
            text: text.to_owned(),
        }),
    }
}

macro_rules! prefixed {
    ($(#[$doc:meta])* $name:ident, $prefix:literal) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// The whole value, prefix included.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Prefixed for $name {
            const PREFIX: &'static str = $prefix;
        }

        impl FromStr for $name {
            type Err = IdError;

            fn from_str(text: &str) -> Result<Self, IdError> {
                parse_prefixed(text, $prefix)?;
                Ok(Self(text.to_owned()))
            }
        }

        string_serde!(
            $name,
            stringify!($name),
            concat!("Starts with \"", $prefix, "\"; at most id_bytes_max (64) bytes."),
            Some(concat!("^", $prefix, "[a-z0-9][a-z0-9_-]*$")),
            Some(Limit::IdBytes)
        );
    };
}

prefixed!(
    /// A node's stable key (`n_`), minted once and kept through renames, moves, copies,
    /// and exports.
    NodeKey, "n_"
);
prefixed!(
    /// A role's stable key (`r_`).
    RoleKey, "r_"
);
prefixed!(
    /// A participation kind's stable key (`k_`). The built-in `owner` kind is `k_owner`.
    KindKey, "k_"
);
prefixed!(
    /// An entity's key (`e_`): deployment-scoped, so one person is one entity everywhere.
    EntityKey, "e_"
);
prefixed!(
    /// An attachment's key (`a_`): a resource, note, or link.
    AttachmentKey, "a_"
);
prefixed!(
    /// A journey's id (`j_`), client-generated and never reused (A19).
    JourneyId, "j_"
);
prefixed!(
    /// A patch's client-generated id (`p_`), the H5 safe-retry handle.
    PatchId, "p_"
);
prefixed!(
    /// A proposal's client-generated id (`pr_`, I6).
    ProposalId, "pr_"
);
prefixed!(
    /// An authenticated user's id (`u_`), the actor on every change (H2).
    UserId, "u_"
);
prefixed!(
    /// An agent's id (`ag_`): the agent token a change was made through, acting for a user.
    AgentId, "ag_"
);

impl KindKey {
    /// The built-in `owner` kind's key.
    #[must_use]
    pub fn owner() -> Self {
        Self("k_owner".to_owned())
    }
}

/// The built-in `owner` kind's id.
#[must_use]
pub fn owner_kind_id() -> Slug {
    Slug("owner".to_owned())
}

/// A route's id: a slug, readable in files, unique in the deployment.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RouteId(Slug);

impl RouteId {
    /// The id.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl FromStr for RouteId {
    type Err = IdError;

    fn from_str(text: &str) -> Result<Self, IdError> {
        Ok(Self(text.parse()?))
    }
}

string_serde!(
    RouteId,
    "RouteId",
    "A route id: a slug, unique in the deployment.",
    Some(SLUG_PATTERN),
    Some(Limit::IdBytes)
);

/// Mints key bodies (PRD, Identity and references: a key is assigned once, when its object
/// is first created). The engine mints through this trait so the host decides how: random
/// bodies in production, a counter in tests and fixtures. A body must be a slug; the caller
/// checks the minted key against every key the graph holds or has retired.
pub trait KeyAllocator {
    /// The next body for a key with `prefix`.
    fn next_body(&mut self, prefix: &'static str) -> String;
}

/// Mints a key of type `K` from `allocator`.
///
/// # Panics
///
/// When the allocator returns a body that is not a slug: a bug in the allocator.
pub fn mint<K: Prefixed>(allocator: &mut dyn KeyAllocator) -> K {
    let body = allocator.next_body(K::PREFIX);
    match K::from_body(&body) {
        Ok(key) => key,
        Err(error) => panic!("key allocator returned an invalid body {body:?}: {error}"),
    }
}

/// A deterministic allocator: one counter per prefix, bodies `1`, `2`, ...
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SequentialKeys {
    counters: std::collections::BTreeMap<&'static str, u64>,
}

impl KeyAllocator for SequentialKeys {
    fn next_body(&mut self, prefix: &'static str) -> String {
        let counter = self.counters.entry(prefix).or_insert(0);
        *counter += 1;
        counter.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        for good in ["setup", "eval_owner", "final-review", "2nd", "n_7q4m2k"] {
            assert!(good.parse::<Slug>().is_ok(), "{good}");
        }
        for bad in ["", "Setup", "-x", "_x", "a b", "a/b", "a.b"] {
            assert!(bad.parse::<Slug>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn slug_at_and_past_its_limit() {
        assert!("s".repeat(64).parse::<Slug>().is_ok());
        let past = "s".repeat(65).parse::<Slug>().unwrap_err();
        assert!(matches!(past, IdError::TooLong(e) if e.limit == Limit::IdBytes));
    }

    #[test]
    fn keys_need_their_own_prefix() {
        assert!("n_7q4m2k".parse::<NodeKey>().is_ok());
        assert!("r_owner".parse::<RoleKey>().is_ok());
        // An id, or a key of another type, is not a node key.
        for bad in ["access", "r_owner", "k_owner", "n_", "n_Bad", "N_x"] {
            assert!(bad.parse::<NodeKey>().is_err(), "{bad}");
        }
        assert!("p_1".parse::<ProposalId>().is_err());
        assert!("pr_1".parse::<PatchId>().is_err());
    }

    #[test]
    fn sequential_keys_count_per_prefix() {
        let mut keys = SequentialKeys::default();
        let first: NodeKey = mint(&mut keys);
        let role: RoleKey = mint(&mut keys);
        let second: NodeKey = mint(&mut keys);
        assert_eq!(
            [first.as_str(), role.as_str(), second.as_str()],
            ["n_1", "r_1", "n_2"]
        );
    }

    #[test]
    fn minted_bodies_take_the_prefix() {
        let key = NodeKey::from_body("0001").unwrap();
        assert_eq!(key.as_str(), "n_0001");
        assert!(NodeKey::from_body("Bad").is_err());
    }

    #[test]
    fn paths_serialize_slash_joined() {
        let path: Path = "testing/partner/criteria".parse().unwrap();
        assert_eq!(path.depth(), 3);
        assert_eq!(path.id().as_str(), "criteria");
        assert_eq!(path.parent().unwrap().to_string(), "testing/partner");
        assert_eq!(
            serde_json::to_string(&path).unwrap(),
            "\"testing/partner/criteria\""
        );
        let root: Path = "testing".parse().unwrap();
        assert_eq!(root.parent(), None);
        assert_eq!(
            root.child("plan".parse().unwrap()).unwrap().to_string(),
            "testing/plan"
        );
        for bad in ["", "a//b", "/a", "a/", "a/B"] {
            assert!(bad.parse::<Path>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn path_depth_at_and_past_its_limit() {
        let at = vec!["a"; 16].join("/");
        let deepest: Path = at.parse().unwrap();
        let past = deepest.child("a".parse().unwrap()).unwrap_err();
        assert!(matches!(past, IdError::TooLong(e) if e.limit == Limit::ContainmentDepth));
        assert!(format!("{at}/a").parse::<Path>().is_err());
    }

    #[test]
    fn paths_sort_depth_first_with_siblings_by_id() {
        let mut paths: Vec<Path> = ["a-b", "a/c", "a", "b", "a/b"]
            .iter()
            .map(|text| text.parse().unwrap())
            .collect();
        paths.sort();
        let sorted: Vec<String> = paths.iter().map(ToString::to_string).collect();
        assert_eq!(sorted, ["a", "a/b", "a/c", "a-b", "b"]);
    }
}
