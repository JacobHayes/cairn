//! The OpenAPI document (ARCHITECTURE, HTTP API; Generated artifacts): generated from the
//! Rust types every request and response is, so it cannot describe a shape the server does
//! not send. Each endpoint's operation is declared here once, beside the endpoint table
//! the router serves; the schemas are the types' own JSON Schemas (draft 2020-12, the
//! dialect of OpenAPI 3.1), so the schema crate's types need no second derive.
//! `mise run gen` writes it to `openapi/`, and the TypeScript client types are generated
//! from it.

use schemars::generate::SchemaSettings;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde_json::{Map, Value, json};

use crate::endpoints::Endpoint;
use crate::query::{self, ParamSpec, schema_of};
use crate::wire::{
    AgentToken, Capabilities, EventPage, JourneyPage, MintedToken, PatchAnswer, PatchRequest,
    Problem, RouteDetail, SearchPage, Tick, TokenRequest, Viewer,
};
use cairn_schema::{
    AgentId, Deployment, DomainDocument, Entity, EntityKey, Journey, JourneyId, Rejection, Route,
    RouteId, RouteVersion, VersionNumber,
};

type SchemaFn = fn(&mut SchemaGenerator) -> Schema;

/// What a successful answer carries.
#[derive(Clone, Copy)]
enum Success {
    /// JSON of a type, at a status.
    Json(u16, SchemaFn),
    /// Nothing (204).
    Empty,
    /// An SSE stream of ticks.
    Events,
}

/// One endpoint's operation.
struct Operation {
    endpoint: &'static Endpoint,
    summary: &'static str,
    path: &'static [(&'static str, &'static str, SchemaFn)],
    query: &'static [ParamSpec],
    body: Option<SchemaFn>,
    success: Success,
    /// A domain patch: answered 409 or 422 with a rejection.
    patch: bool,
}

const fn operation(endpoint: &'static Endpoint, summary: &'static str) -> Operation {
    Operation {
        endpoint,
        summary,
        path: &[],
        query: &[],
        body: None,
        success: Success::Empty,
        patch: false,
    }
}

const JOURNEY_ID: (&str, &str, SchemaFn) = ("id", "The journey.", schema_of::<JourneyId>);
const ROUTE_ID: (&str, &str, SchemaFn) = ("id", "The route.", schema_of::<RouteId>);

fn json<T: JsonSchema>() -> Success {
    Success::Json(200, schema_of::<T>)
}

fn patch(endpoint: &'static Endpoint, summary: &'static str) -> Operation {
    Operation {
        body: Some(schema_of::<PatchRequest>),
        success: json::<PatchAnswer>(),
        patch: true,
        ..operation(endpoint, summary)
    }
}

fn read<T: JsonSchema>(endpoint: &'static Endpoint, summary: &'static str) -> Operation {
    Operation {
        success: json::<T>(),
        ..operation(endpoint, summary)
    }
}

/// Every endpoint's operation, in the endpoint table's order.
fn operations() -> Vec<Operation> {
    let mut all = writes();
    all.extend(reads());
    all.extend(users());
    all
}

/// Capabilities and the domain patches.
fn writes() -> Vec<Operation> {
    use crate::endpoints as at;
    vec![
        read::<Capabilities>(&at::CAPABILITIES, "What this host offers."),
        Operation {
            path: &[JOURNEY_ID],
            ..patch(
                &at::PATCH_JOURNEY,
                "Applies a journey patch; base revision 0 creates it (A17).",
            )
        },
        Operation {
            path: &[ROUTE_ID],
            ..patch(
                &at::PATCH_ROUTE,
                "Applies a route patch, its draft included (A17).",
            )
        },
        patch(
            &at::PATCH_DEPLOYMENT,
            "Applies a deployment patch: entities and merges (E6).",
        ),
    ]
}

/// The store-backed reads.
fn reads() -> Vec<Operation> {
    use crate::endpoints as at;
    vec![
        Operation {
            query: query::JOURNEY_PARAMS,
            ..read::<JourneyPage>(
                &at::JOURNEYS,
                "The journey index, filtered and paged (C16).",
            )
        },
        Operation {
            path: &[JOURNEY_ID],
            ..read::<Journey>(&at::JOURNEY, "A journey with its graph and state.")
        },
        Operation {
            path: &[JOURNEY_ID],
            ..read::<DomainDocument>(
                &at::DOCUMENT,
                "The domain document: the journey, the caller's derive inputs, and the engine \
                 version; nothing derived.",
            )
        },
        Operation {
            path: &[ROUTE_ID],
            ..read::<Route>(&at::ROUTE, "A route with its draft.")
        },
        Operation {
            path: &[ROUTE_ID],
            ..read::<RouteDetail>(
                &at::ROUTE_VERSIONS,
                "A route's versions and their journeys (C17).",
            )
        },
        Operation {
            path: &[
                ROUTE_ID,
                ("version", "The version.", schema_of::<VersionNumber>),
            ],
            ..read::<RouteVersion>(&at::ROUTE_VERSION, "One published route version.")
        },
        read::<Deployment>(
            &at::DEPLOYMENT,
            "The deployment: entities and aliases (E6).",
        ),
        Operation {
            path: &[(
                "key",
                "The entity, or an alias of it.",
                schema_of::<EntityKey>,
            )],
            ..read::<Entity>(&at::ENTITY, "An entity, resolved through its aliases (E6).")
        },
        Operation {
            query: query::SEARCH_PARAMS,
            ..read::<SearchPage>(&at::SEARCH, "Text search across journeys, paged.")
        },
        Operation {
            query: query::EVENT_PARAMS,
            ..read::<EventPage>(
                &at::EVENTS,
                "Events, filtered and paged in commit order (J5).",
            )
        },
    ]
}

/// The stream, the caller, and their agent tokens.
fn users() -> Vec<Operation> {
    use crate::endpoints as at;
    vec![
        Operation {
            query: query::STREAM_PARAMS,
            success: Success::Events,
            ..operation(
                &at::STREAM,
                "Revision ticks as server-sent `tick` events, current revisions first (H6).",
            )
        },
        read::<Viewer>(&at::VIEWER, "The caller and their entities (H2, H3)."),
        read::<Vec<AgentToken>>(&at::TOKENS, "The caller's agent tokens."),
        Operation {
            body: Some(schema_of::<TokenRequest>),
            success: Success::Json(201, schema_of::<MintedToken>),
            ..operation(
                &at::MINT_TOKEN,
                "Mints an agent token, shown this once (H2).",
            )
        },
        Operation {
            path: &[("agent", "The token's agent.", schema_of::<AgentId>)],
            ..operation(
                &at::REVOKE_TOKEN,
                "Revokes one of the caller's agent tokens.",
            )
        },
    ]
}

/// The OpenAPI document, pretty-printed with a trailing newline.
///
/// # Panics
///
/// Never: a document of JSON values always serializes.
#[must_use]
pub fn document_text() -> String {
    let mut text = match serde_json::to_string_pretty(&document()) {
        Ok(text) => text,
        Err(error) => unreachable!("a JSON value always serializes: {error}"),
    };
    text.push('\n');
    text
}

/// The OpenAPI document.
#[must_use]
pub fn document() -> Value {
    let mut generator = SchemaSettings::draft2020_12()
        .with(|settings| settings.definitions_path = "/components/schemas".into())
        .into_generator();
    let problem = generator.subschema_for::<Problem>();
    let rejection = generator.subschema_for::<Rejection>();
    let tick = generator.subschema_for::<Tick>();
    let mut paths = Map::new();
    for operation in operations() {
        let rendered = render(&operation, &mut generator, &problem, &rejection, &tick);
        let item = paths
            .entry(operation.endpoint.path)
            .or_insert_with(|| json!({}));
        let method = operation.endpoint.method.as_str().to_ascii_lowercase();
        if let Value::Object(item) = item {
            item.insert(method, rendered);
        }
    }
    let schemas = generator.take_definitions(true);
    json!({
        "openapi": "3.1.0",
        "x-generated": "Generated by `mise run gen` from crates/api (cairn_api::openapi); do not edit by hand.",
        "info": {
            "title": "Cairn",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "The Cairn HTTP API. Every request is authenticated (a session cookie or a bearer token). A rejected patch answers its rejection unchanged, every violation by path (A15); other errors answer a Problem with the request id.",
        },
        "paths": paths,
        "components": {
            "schemas": schemas,
            "securitySchemes": {"bearer": {"type": "http", "scheme": "bearer"}},
        },
        "security": [{"bearer": []}],
    })
}

/// One operation as OpenAPI writes it.
fn render(
    operation: &Operation,
    generator: &mut SchemaGenerator,
    problem: &Schema,
    rejection: &Schema,
    tick: &Schema,
) -> Value {
    let mut parameters: Vec<Value> = operation
        .path
        .iter()
        .map(|(name, description, schema)| {
            json!({"name": name, "in": "path", "required": true, "description": description, "schema": schema(generator)})
        })
        .collect();
    parameters.extend(
        operation
            .query
            .iter()
            .map(|spec| query_parameter(spec, generator)),
    );
    let mut rendered = json!({
        "operationId": operation.endpoint.operation,
        "summary": operation.summary,
        "responses": responses(operation, generator, problem, rejection, tick),
    });
    if !parameters.is_empty() {
        rendered["parameters"] = Value::Array(parameters);
    }
    if let Some(body) = operation.body {
        rendered["requestBody"] = json!({
            "required": true,
            "content": {"application/json": {"schema": body(generator)}},
        });
    }
    rendered
}

fn query_parameter(spec: &ParamSpec, generator: &mut SchemaGenerator) -> Value {
    let one = (spec.schema)(generator);
    let schema = if spec.repeated {
        json!({"type": "array", "items": one})
    } else {
        one.to_value()
    };
    json!({
        "name": spec.name,
        "in": "query",
        "required": spec.required,
        "description": spec.description,
        "schema": schema,
        "style": "form",
        "explode": true,
    })
}

fn responses(
    operation: &Operation,
    generator: &mut SchemaGenerator,
    problem: &Schema,
    rejection: &Schema,
    tick: &Schema,
) -> Value {
    let json_of = |schema: &Schema, description: &str| json!({"description": description, "content": {"application/json": {"schema": schema}}});
    let mut answers = Map::new();
    match operation.success {
        Success::Json(status, schema) => {
            answers.insert(status.to_string(), json_of(&schema(generator), "Answered."));
        }
        Success::Empty => {
            answers.insert("204".to_owned(), json!({"description": "Done."}));
        }
        Success::Events => {
            let description = format!(
                "An SSE stream: `{}` events whose data is a Tick (schema below), and \
                 heartbeat comments.",
                crate::stream::TICK_EVENT
            );
            let events = json!({"description": description, "content": {"text/event-stream": {"schema": {"type": "string"}, "x-event-data": tick}}});
            answers.insert("200".to_owned(), events);
        }
    }
    let mut problems = vec![
        ("400", "The request is malformed."),
        (
            "500",
            "The server failed; the request id names it in the logs.",
        ),
        (
            "503",
            "A limit was reached or the request timed out; retry after `Retry-After`.",
        ),
    ];
    if !operation.path.is_empty() {
        problems.push(("404", "No such resource."));
    }
    if operation.body.is_some() {
        problems.push(("413", "The body is over the request size limit."));
        problems.push(("415", "The body is not application/json."));
    }
    if matches!(
        operation.endpoint.path,
        "/users/me/tokens" | "/users/me/tokens/{agent}"
    ) {
        problems.push((
            "403",
            "Only a user may do this, not an agent acting for one.",
        ));
    }
    for (status, description) in problems {
        answers.insert(status.to_owned(), json_of(problem, description));
    }
    auth_refusals(&mut answers);
    if operation.patch {
        let stale = "Stale (with what intervened) or a reused patch id (H5).";
        answers.insert("409".to_owned(), json_of(rejection, stale));
        let invalid = "Invalid: every violation, by path (A15).";
        answers.insert("422".to_owned(), json_of(rejection, invalid));
    }
    Value::Object(answers)
}

/// The auth layer's own answers, which stand in front of every operation in plain text
/// (3.2): 401 with its `WWW-Authenticate` challenge, 403 for a peer a local-only provider
/// refuses, and 503 when an identity provider cannot be asked (no `Retry-After`). A status
/// the API also answers as a problem gets the text body beside it.
fn auth_refusals(answers: &mut Map<String, Value>) {
    let refusals = [
        (
            "401",
            "No credential, or one that is refused (the auth layer, in text).",
        ),
        (
            "403",
            "The auth layer refused the peer: a local-only provider and a remote peer (in text).",
        ),
        (
            "503",
            "The auth layer could not ask an identity provider (in text, without `Retry-After`).",
        ),
    ];
    for (status, description) in refusals {
        let text = json!({"schema": {"type": "string"}});
        let Some(answer) = answers.get_mut(status) else {
            let answer = json!({"description": description, "content": {"text/plain": text}});
            answers.insert(status.to_owned(), answer);
            continue;
        };
        let joined = format!(
            "{} Or: {description}",
            answer["description"].as_str().unwrap_or("")
        );
        answer["description"] = Value::String(joined);
        answer["content"]["text/plain"] = text;
    }
}

/// The endpoints the document describes, by operation id.
#[must_use]
pub fn operation_ids() -> Vec<&'static str> {
    operations()
        .iter()
        .map(|operation| operation.endpoint.operation)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_endpoint_has_one_operation_in_table_order() {
        let table: Vec<&str> = crate::endpoints::ALL
            .iter()
            .map(|endpoint| endpoint.operation)
            .collect();
        assert_eq!(operation_ids(), table);
    }

    #[test]
    fn every_reference_names_a_component() {
        let document = document();
        let text = document.to_string();
        let mut rest = text.as_str();
        let mut seen = 0;
        while let Some(start) = rest.find("\"$ref\":\"#/components/schemas/") {
            let after = &rest[start + "\"$ref\":\"#/components/schemas/".len()..];
            let name = &after[..after.find('"').unwrap()];
            assert!(
                document["components"]["schemas"].get(name).is_some(),
                "{name}"
            );
            seen += 1;
            rest = after;
        }
        assert!(seen > 0);
    }
}
