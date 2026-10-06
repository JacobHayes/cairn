//! Query parameters: each endpoint's are declared once, as [`ParamSpec`]s the handler parses
//! by and the OpenAPI document describes, and a parameter an endpoint does not declare is
//! refused rather than ignored. A repeated parameter is given once per value
//! (`?status=active&status=archived`). Pages take `after` (the previous page's `next`) and
//! `size` (1 to the page limit; larger sizes are cut to it).

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use cairn_schema::{
    Domain, EntityKey, EventType, JourneyId, JourneyStatus, NodeKey, PatchId, ProposalId,
    RevisionOf, RouteId, Timestamp, Title, UserId, VersionNumber,
};
use cairn_store::{EventQuery, JourneyQuery, PageSize, SearchQuery, Watch};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::extract::parse_text;

/// One query parameter an endpoint takes.
#[derive(Clone, Copy, Debug)]
pub struct ParamSpec {
    /// Its name.
    pub name: &'static str,
    /// What it does.
    pub description: &'static str,
    /// Whether it may be given more than once.
    pub repeated: bool,
    /// Whether the endpoint needs it.
    pub required: bool,
    /// The schema of one value.
    pub schema: fn(&mut SchemaGenerator) -> Schema,
}

/// The schema of `T`, as a reference to its component when it has a name.
pub fn schema_of<T: JsonSchema>(generator: &mut SchemaGenerator) -> Schema {
    generator.subschema_for::<T>()
}

impl ParamSpec {
    const fn one<T: JsonSchema>(name: &'static str, description: &'static str) -> Self {
        Self {
            name,
            description,
            repeated: false,
            required: false,
            schema: schema_of::<T>,
        }
    }

    const fn many<T: JsonSchema>(name: &'static str, description: &'static str) -> Self {
        Self {
            repeated: true,
            ..Self::one::<T>(name, description)
        }
    }

    const fn needed<T: JsonSchema>(name: &'static str, description: &'static str) -> Self {
        Self {
            required: true,
            ..Self::one::<T>(name, description)
        }
    }
}

const SIZE: ParamSpec = ParamSpec::one::<u32>(
    "size",
    "Items per page, 1 to the page limit (200); larger sizes are cut to it.",
);

/// `GET /journeys` (C16).
pub const JOURNEY_PARAMS: &[ParamSpec] = &[
    ParamSpec::many::<JourneyStatus>("status", "Journeys in any of these statuses."),
    ParamSpec::one::<RouteId>("route", "Journeys following this route."),
    ParamSpec::one::<VersionNumber>("version", "Journeys on this version of their route."),
    ParamSpec::many::<EntityKey>(
        "referencing",
        "Journeys referring to any of these entities, directly or through an alias (E6).",
    ),
    ParamSpec::one::<bool>(
        "upgrade_available",
        "Journeys whose route has, or has not, published a newer version.",
    ),
    ParamSpec::one::<JourneyId>("after", "The page starts after this journey."),
    SIZE,
];

/// `GET /search`.
pub const SEARCH_PARAMS: &[ParamSpec] = &[
    ParamSpec::needed::<Title>(
        "text",
        "What to find in journey names and descriptions, node titles and descriptions, notes, \
         and resources; ASCII letters match in either case.",
    ),
    ParamSpec::one::<JourneyId>("after", "The page starts after this journey."),
    SIZE,
];

/// `GET /events` (J5).
pub const EVENT_PARAMS: &[ParamSpec] = &[
    ParamSpec::one::<DomainName>("log", "Events in this domain's log."),
    ParamSpec::one::<NodeKey>("node", "Events that wrote anything on this node."),
    ParamSpec::one::<UserId>("user", "Events whose actor is this user."),
    ParamSpec::many::<EventType>("type", "Events of any of these types."),
    ParamSpec::one::<PatchId>("patch", "Events of this patch."),
    ParamSpec::one::<Timestamp>("from", "Events committed at or after this time."),
    ParamSpec::one::<Timestamp>("until", "Events committed before this time."),
    ParamSpec::one::<u64>("after", "The page starts after this position in the log."),
    SIZE,
];

/// `GET /events/stream` (H6).
pub const STREAM_PARAMS: &[ParamSpec] = &[ParamSpec {
    required: true,
    ..ParamSpec::many::<WatchName>(
        "domain",
        "What to watch: `deployment`, `journey:<id>`, `route:<id>`, or `proposal:<id>` for \
         one, or `journeys`, `routes`, or `proposals` for every one of a kind.",
    )
}];

/// A request's query parameters, checked against what its endpoint declares.
#[derive(Debug)]
pub struct Params {
    pairs: Vec<(String, String)>,
}

impl Params {
    /// Parses `query`, refusing a parameter not in `specs`, one given twice that is not
    /// repeated, and a missing required one.
    ///
    /// # Errors
    ///
    /// A bad request naming the parameter.
    pub fn parse(query: Option<&str>, specs: &[ParamSpec]) -> Result<Self, ApiError> {
        let pairs: Vec<(String, String)> =
            url::form_urlencoded::parse(query.unwrap_or_default().as_bytes())
                .map(|(name, value)| (name.into_owned(), value.into_owned()))
                .collect();
        for (name, _) in &pairs {
            if !specs.iter().any(|spec| spec.name == name) {
                return Err(ApiError::bad_request(format!(
                    "no query parameter {name:?} here"
                )));
            }
        }
        for spec in specs {
            let given = pairs.iter().filter(|(name, _)| name == spec.name).count();
            if given > 1 && !spec.repeated {
                return Err(ApiError::bad_request(format!(
                    "{} is given more than once",
                    spec.name
                )));
            }
            if given == 0 && spec.required {
                return Err(ApiError::bad_request(format!("{} is required", spec.name)));
            }
        }
        Ok(Self { pairs })
    }

    /// Every value of `name`, in order.
    ///
    /// # Errors
    ///
    /// A bad request naming the parameter and the value that does not parse.
    pub fn all<T: DeserializeOwned>(&self, name: &str) -> Result<Vec<T>, ApiError> {
        let values = self.pairs.iter().filter(|(given, _)| given == name);
        values
            .map(|(_, value)| {
                parse_text(value)
                    .map_err(|reason| ApiError::bad_request(format!("{name}={value:?}: {reason}")))
            })
            .collect()
    }

    /// The value of `name`, if given.
    ///
    /// # Errors
    ///
    /// A bad request when it does not parse.
    pub fn one<T: DeserializeOwned>(&self, name: &str) -> Result<Option<T>, ApiError> {
        Ok(self.all(name)?.into_iter().next())
    }

    fn size(&self) -> Result<PageSize, ApiError> {
        Ok(self
            .one::<u32>("size")?
            .map_or(PageSize::MAX, PageSize::new))
    }
}

/// C16: the journey index query.
///
/// # Errors
///
/// A bad request for a parameter that does not parse.
pub fn journeys(params: &Params) -> Result<JourneyQuery, ApiError> {
    let referencing: Vec<EntityKey> = params.all("referencing")?;
    Ok(JourneyQuery {
        statuses: params.all("status")?.into_iter().collect(),
        route: params.one("route")?,
        version: params.one("version")?,
        referencing: (!referencing.is_empty()).then(|| referencing.into_iter().collect()),
        upgrade_available: params.one("upgrade_available")?,
        after: params.one("after")?,
        size: params.size()?,
    })
}

/// The search query.
///
/// # Errors
///
/// A bad request for a parameter that does not parse.
pub fn search(params: &Params) -> Result<SearchQuery, ApiError> {
    let text: Option<Title> = params.one("text")?;
    let Some(text) = text else {
        return Err(ApiError::bad_request("text is required"));
    };
    Ok(SearchQuery {
        text,
        after: params.one("after")?,
        size: params.size()?,
    })
}

/// J5: the event query.
///
/// # Errors
///
/// A bad request for a parameter that does not parse.
pub fn events(params: &Params) -> Result<EventQuery, ApiError> {
    let log: Option<DomainName> = params.one("log")?;
    Ok(EventQuery {
        log: log.map(|name| name.0),
        node: params.one("node")?,
        user: params.one("user")?,
        types: params.all("type")?.into_iter().collect(),
        patch: params.one("patch")?,
        from: params.one("from")?,
        until: params.one("until")?,
        after: params.one("after")?,
        size: params.size()?,
    })
}

/// H6: what a stream watches.
///
/// # Errors
///
/// A bad request for a value that does not parse.
pub fn watching(params: &Params) -> Result<BTreeSet<Watch>, ApiError> {
    let names: Vec<WatchName> = params.all("domain")?;
    Ok(names.into_iter().map(|name| name.0).collect())
}

/// A domain named in a query: `deployment`, `journey:<id>`, or `route:<id>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainName(pub Domain);

impl FromStr for DomainName {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let domain = match text.split_once(':') {
            None if text == "deployment" => Domain::Deployment,
            Some(("journey", id)) => {
                Domain::Journey(id.parse().map_err(|error| format!("{error}"))?)
            }
            Some(("route", id)) => Domain::Route(id.parse().map_err(|error| format!("{error}"))?),
            _ => {
                return Err(format!(
                    "{text:?} is not deployment, journey:<id>, or route:<id>"
                ));
            }
        };
        Ok(Self(domain))
    }
}

impl fmt::Display for DomainName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Domain::Deployment => formatter.write_str("deployment"),
            Domain::Journey(id) => write!(formatter, "journey:{id}"),
            Domain::Route(id) => write!(formatter, "route:{id}"),
        }
    }
}

/// What a stream watches, as a query names it: one domain or proposal (`deployment`,
/// `journey:<id>`, `route:<id>`, `proposal:<id>`), or every one of a kind (`journeys`,
/// `routes`, `proposals`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchName(pub Watch);

impl FromStr for WatchName {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let watch = match text {
            "journeys" => Watch::Journeys,
            "routes" => Watch::Routes,
            "proposals" => Watch::Proposals,
            _ => match text.split_once(':') {
                Some(("proposal", id)) => {
                    let id: ProposalId = id.parse().map_err(|error| format!("{error}"))?;
                    Watch::One(RevisionOf::Proposal(id))
                }
                _ => Watch::One(RevisionOf::Domain(text.parse::<DomainName>()?.0)),
            },
        };
        Ok(Self(watch))
    }
}

impl fmt::Display for WatchName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Watch::Journeys => formatter.write_str("journeys"),
            Watch::Routes => formatter.write_str("routes"),
            Watch::Proposals => formatter.write_str("proposals"),
            Watch::One(RevisionOf::Proposal(id)) => write!(formatter, "proposal:{id}"),
            Watch::One(RevisionOf::Domain(domain)) => DomainName(domain.clone()).fmt(formatter),
        }
    }
}

/// Serde and JSON Schema for a name written as a string with a pattern.
macro_rules! named_in_text {
    ($type:ty, $name:literal, $description:literal, $pattern:literal) => {
        impl Serialize for $type {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $type {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text = String::deserialize(deserializer)?;
                text.parse().map_err(serde::de::Error::custom)
            }
        }

        impl JsonSchema for $type {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                $name.into()
            }

            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                schemars::json_schema!({
                    "type": "string",
                    "description": $description,
                    "pattern": $pattern,
                })
            }
        }
    };
}

named_in_text!(
    DomainName,
    "DomainName",
    "A domain: `deployment`, `journey:<id>`, or `route:<id>`.",
    "^(deployment|journey:.+|route:.+)$"
);
named_in_text!(
    WatchName,
    "WatchName",
    "What a stream watches: `deployment`, `journey:<id>`, `route:<id>`, `proposal:<id>`, \
     `journeys`, `routes`, or `proposals`.",
    "^(deployment|journeys|routes|proposals|(journey|route|proposal):.+)$"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_through_text() {
        for text in [
            "deployment",
            "journey:j_one",
            "route:r_one",
            "proposal:pr_one",
            "journeys",
            "routes",
            "proposals",
        ] {
            let name: WatchName = text.parse().unwrap();
            assert_eq!(name.to_string(), text);
        }
        for text in ["journey", "node:n_a", "journey:", ""] {
            assert!(text.parse::<WatchName>().is_err(), "{text:?}");
        }
    }

    #[test]
    fn parameters_are_checked_against_what_the_endpoint_declares() {
        let refused = [
            "colour=blue",
            "route=r_a&route=r_b",
            "version=two",
            "status=paused",
        ];
        for query in refused {
            let checked = Params::parse(Some(query), JOURNEY_PARAMS).and_then(|p| journeys(&p));
            assert!(checked.is_err(), "{query}");
        }
        let params = Params::parse(
            Some("status=active&status=archived&size=500"),
            JOURNEY_PARAMS,
        )
        .unwrap();
        let query = journeys(&params).unwrap();
        assert_eq!(query.statuses.len(), 2);
        assert_eq!(query.size, PageSize::MAX);
        assert!(
            Params::parse(None, SEARCH_PARAMS).is_err(),
            "text is required"
        );
    }
}
