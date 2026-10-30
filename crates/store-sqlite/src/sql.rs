//! Statements and values: the thin layer between the store's records and SQLite's rows.

use std::str::FromStr;

use cairn_schema::{Domain, GraphId, Timestamp};
use cairn_store::{CommitError, StoreError};
use rusqlite::types::Value;
use rusqlite::{Connection, ErrorCode, params_from_iter};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// A failed statement, classified: a constraint (a deferred foreign key fails the `COMMIT`
/// that leaves a row without its parent), or anything else.
#[derive(Debug)]
pub(crate) enum SqlError {
    Constraint(String),
    Other(String),
}

impl From<rusqlite::Error> for SqlError {
    fn from(error: rusqlite::Error) -> Self {
        match error.sqlite_error_code() {
            Some(ErrorCode::ConstraintViolation) => SqlError::Constraint(error.to_string()),
            _ => SqlError::Other(error.to_string()),
        }
    }
}

impl From<SqlError> for StoreError {
    fn from(error: SqlError) -> Self {
        match error {
            SqlError::Constraint(text) | SqlError::Other(text) => StoreError::Backend(text),
        }
    }
}

impl From<SqlError> for CommitError {
    fn from(error: SqlError) -> Self {
        CommitError::Failed(error.into())
    }
}

/// One row, as values by position.
pub(crate) struct Row(Vec<Value>);

impl Row {
    fn value(&self, index: usize) -> &Value {
        self.0.get(index).unwrap_or(&Value::Null)
    }

    pub fn text(&self, index: usize) -> Result<String, StoreError> {
        self.opt_text(index)?
            .ok_or_else(|| corrupt(&format!("column {index} is null")))
    }

    pub fn opt_text(&self, index: usize) -> Result<Option<String>, StoreError> {
        match self.value(index) {
            Value::Null => Ok(None),
            Value::Text(text) => Ok(Some(text.clone())),
            other => Err(corrupt(&format!("column {index} is {other:?}, not text"))),
        }
    }

    pub fn int(&self, index: usize) -> Result<i64, StoreError> {
        self.opt_int(index)?
            .ok_or_else(|| corrupt(&format!("column {index} is null")))
    }

    pub fn opt_int(&self, index: usize) -> Result<Option<i64>, StoreError> {
        match self.value(index) {
            Value::Null => Ok(None),
            Value::Integer(number) => Ok(Some(*number)),
            other => Err(corrupt(&format!(
                "column {index} is {other:?}, not an integer"
            ))),
        }
    }

    pub fn parse<T: FromStr>(&self, index: usize) -> Result<T, StoreError>
    where
        T::Err: std::fmt::Display,
    {
        parse(&self.text(index)?)
    }

    pub fn opt_parse<T: FromStr>(&self, index: usize) -> Result<Option<T>, StoreError>
    where
        T::Err: std::fmt::Display,
    {
        self.opt_text(index)?.map(|text| parse(&text)).transpose()
    }

    pub fn number<T: TryFrom<u32>>(&self, index: usize) -> Result<T, StoreError>
    where
        T::Error: std::fmt::Display,
    {
        number(self.int(index)?)
    }

    pub fn timestamp(&self, index: usize) -> Result<Timestamp, StoreError> {
        timestamp(self.int(index)?)
    }

    pub fn json<T: DeserializeOwned>(&self, index: usize) -> Result<T, StoreError> {
        serde_json::from_str(&self.text(index)?)
            .map_err(|error| corrupt(&format!("column {index}: {error}")))
    }

    pub fn opt_json<T: DeserializeOwned>(&self, index: usize) -> Result<Option<T>, StoreError> {
        self.opt_text(index)?
            .map(|text| serde_json::from_str(&text))
            .transpose()
            .map_err(|error| corrupt(&format!("column {index}: {error}")))
    }

    pub fn flag(&self, index: usize) -> Result<bool, StoreError> {
        Ok(self.int(index)? != 0)
    }

    pub fn opt_flag(&self, index: usize) -> Result<Option<bool>, StoreError> {
        Ok(self.opt_int(index)?.map(|flag| flag != 0))
    }

    pub fn opt_timestamp(&self, index: usize) -> Result<Option<Timestamp>, StoreError> {
        self.opt_int(index)?.map(timestamp).transpose()
    }

    /// A unit enum from its name (see [`enum_name`]).
    pub fn name<T: DeserializeOwned>(&self, index: usize) -> Result<T, StoreError> {
        enum_from(&self.text(index)?)
    }
}

pub(crate) fn corrupt(reason: &str) -> StoreError {
    StoreError::Backend(format!("a stored row does not load: {reason}"))
}

pub(crate) fn parse<T: FromStr>(text: &str) -> Result<T, StoreError>
where
    T::Err: std::fmt::Display,
{
    text.parse()
        .map_err(|error| corrupt(&format!("{text:?}: {error}")))
}

pub(crate) fn number<T: TryFrom<u32>>(value: i64) -> Result<T, StoreError>
where
    T::Error: std::fmt::Display,
{
    let narrow = u32::try_from(value).map_err(|error| corrupt(&format!("{value}: {error}")))?;
    T::try_from(narrow).map_err(|error| corrupt(&format!("{value}: {error}")))
}

pub(crate) fn timestamp(nanoseconds: i64) -> Result<Timestamp, StoreError> {
    Timestamp::from_nanosecond(i128::from(nanoseconds))
        .map_err(|error| corrupt(&format!("timestamp {nanoseconds}: {error}")))
}

/// Runs a statement.
pub(crate) fn execute(
    connection: &Connection,
    sql: &str,
    params: Vec<Value>,
) -> Result<u64, SqlError> {
    let changed = connection
        .prepare_cached(sql)?
        .execute(params_from_iter(params))?;
    Ok(u64::try_from(changed).unwrap_or(u64::MAX))
}

/// Runs a query and collects every row.
pub(crate) fn rows(
    connection: &Connection,
    sql: &str,
    params: Vec<Value>,
) -> Result<Vec<Row>, SqlError> {
    let mut statement = connection.prepare_cached(sql)?;
    let count = statement.column_count();
    let mut result = statement.query(params_from_iter(params))?;
    let mut collected = Vec::new();
    while let Some(row) = result.next()? {
        let mut values = Vec::with_capacity(count);
        for index in 0..count {
            values.push(row.get::<_, Value>(index)?);
        }
        collected.push(Row(values));
    }
    Ok(collected)
}

/// Runs a query and takes its first row.
pub(crate) fn first(
    connection: &Connection,
    sql: &str,
    params: Vec<Value>,
) -> Result<Option<Row>, SqlError> {
    Ok(rows(connection, sql, params)?.into_iter().next())
}

/// A text value.
pub(crate) fn text<T: ToString + ?Sized>(value: &T) -> Value {
    Value::Text(value.to_string())
}

/// A text value, or null.
pub(crate) fn opt_text<T: ToString>(value: Option<T>) -> Value {
    value.map_or(Value::Null, |value| Value::Text(value.to_string()))
}

/// An integer value.
pub(crate) fn int(value: impl Into<i64>) -> Value {
    Value::Integer(value.into())
}

/// An integer value, or null.
pub(crate) fn opt_int<T: Into<i64>>(value: Option<T>) -> Value {
    value.map_or(Value::Null, |value| Value::Integer(value.into()))
}

/// A flag: 0 or 1.
pub(crate) fn flag(value: bool) -> Value {
    Value::Integer(i64::from(value))
}

/// A timestamp, as nanoseconds since the epoch.
pub(crate) fn time(value: Timestamp) -> Result<Value, StoreError> {
    i64::try_from(value.as_nanosecond())
        .map(Value::Integer)
        .map_err(|_| StoreError::Malformed(format!("timestamp {value} is out of range")))
}

/// A calendar date or a moment on one, as ISO text.
pub(crate) fn date(value: impl std::fmt::Display) -> Value {
    Value::Text(value.to_string())
}

/// A JSON column.
pub(crate) fn json<T: Serialize>(value: &T) -> Result<Value, StoreError> {
    serde_json::to_string(value)
        .map(Value::Text)
        .map_err(|error| StoreError::Backend(format!("a value does not serialize: {error}")))
}

/// A JSON column, or null.
pub(crate) fn opt_json<T: Serialize>(value: Option<&T>) -> Result<Value, StoreError> {
    value.map_or(Ok(Value::Null), json)
}

/// A timestamp, or null.
pub(crate) fn opt_time(value: Option<Timestamp>) -> Result<Value, StoreError> {
    value.map_or(Ok(Value::Null), time)
}

/// A unit enum's written name, for a text column. Serde's derive names every unit variant,
/// so a unit variant added later round-trips with no change here; a variant that carries
/// data has no name and fails the write.
pub(crate) fn enum_name<T: Serialize>(value: &T) -> Result<String, StoreError> {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(name)) => Ok(name),
        other => Err(StoreError::Backend(format!(
            "{other:?} is not an enum name"
        ))),
    }
}

/// A unit enum's name ([`enum_name`]) as a text value.
pub(crate) fn json_enum<T: Serialize>(value: &T) -> Result<Value, StoreError> {
    enum_name(value).map(Value::Text)
}

/// A unit enum from its written name ([`enum_name`]).
pub(crate) fn enum_from<T: DeserializeOwned>(name: &str) -> Result<T, StoreError> {
    serde_json::from_value(serde_json::Value::String(name.to_owned()))
        .map_err(|error| corrupt(&format!("{name:?}: {error}")))
}

/// A graph's row id: `journey/<id>`, `draft/<route>`, or `version/<route>/<number>`.
pub(crate) fn graph_id(graph: &GraphId) -> String {
    match graph {
        GraphId::Journey(journey) => format!("journey/{journey}"),
        GraphId::RouteDraft(route) => format!("draft/{route}"),
        GraphId::RouteVersion { route, version } => format!("version/{route}/{version}"),
    }
}

/// A graph from its row id.
pub(crate) fn graph_from(id: &str) -> Result<GraphId, StoreError> {
    let parts: Vec<&str> = id.split('/').collect();
    match parts.as_slice() {
        ["journey", journey] => Ok(GraphId::Journey(parse(journey)?)),
        ["draft", route] => Ok(GraphId::RouteDraft(parse(route)?)),
        ["version", route, version] => Ok(GraphId::RouteVersion {
            route: parse(route)?,
            version: number(version.parse().map_err(|_| corrupt(id))?)?,
        }),
        _ => Err(corrupt(&format!("graph id {id:?}"))),
    }
}

/// A domain as its kind and id columns.
pub(crate) fn domain_columns(domain: &Domain) -> (&'static str, Value) {
    match domain {
        Domain::Journey(journey) => ("journey", text(journey)),
        Domain::Route(route) => ("route", text(route)),
        Domain::Deployment => ("deployment", Value::Null),
    }
}

/// A domain from its kind and id columns.
pub(crate) fn domain_from(kind: &str, id: Option<String>) -> Result<Domain, StoreError> {
    match (kind, id) {
        ("journey", Some(id)) => Ok(Domain::Journey(parse(&id)?)),
        ("route", Some(id)) => Ok(Domain::Route(parse(&id)?)),
        ("deployment", None) => Ok(Domain::Deployment),
        (kind, id) => Err(corrupt(&format!("domain {kind} {id:?}"))),
    }
}
