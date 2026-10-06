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
    AgentToken, Capabilities, EventPage, History, JourneyPage, Mine, MintedToken, NodeDetail,
    PatchAnswer, PatchRequest, Problem, Projected, ProposalAnswer, ProposalApply, ProposalCreate,
    ProposalEdit, ProposalReview, ProposalStep, RelinkRequest, RouteDetail, RouteImport,
    SaveAsRouteRequest, SearchPage, Tick, TokenRequest, UpgradeRequest, Viewer,
};
use cairn_schema::{
    AgentId, DecisionView, Deployment, DomainDocument, Entity, EntityKey, ExplainedField,
    ExplanationPage, Journey, JourneyId, Level, ListPage, Next, NodeKey, Proposal, ProposalId,
    Rejection, Route, RouteFile, RouteId, RouteVersion, Snapshot, StatusSummary, Timeline, Trace,
    VersionNumber,
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
    /// A patch underneath: answered 409 or 422 with a rejection.
    patch: bool,
    /// Drafted by the engine: also answered 422 with a problem when it cannot be drafted.
    drafts: bool,
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
        drafts: false,
    }
}

const JOURNEY_ID: (&str, &str, SchemaFn) = ("id", "The journey.", schema_of::<JourneyId>);
const ROUTE_ID: (&str, &str, SchemaFn) = ("id", "The route.", schema_of::<RouteId>);
const PROPOSAL_ID: (&str, &str, SchemaFn) = (
    "id",
    "The proposal, by its client-generated id.",
    schema_of::<ProposalId>,
);
const NODE_KEY: (&str, &str, SchemaFn) = ("key", "The node.", schema_of::<NodeKey>);

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
    all.extend(proposals());
    all.extend(bulk());
    let mut reads = reads();
    // The projections follow the domain document, as in the endpoint table.
    let rest = reads.split_off(3);
    all.extend(reads);
    all.extend(projections());
    all.extend(node_reads());
    all.extend(rest);
    all.extend(users());
    all
}

/// A journey's projection: its id in the path, answered with what it was derived from.
fn projection<T: JsonSchema>(
    endpoint: &'static Endpoint,
    summary: &'static str,
    query: &'static [ParamSpec],
) -> Operation {
    Operation {
        path: &[JOURNEY_ID],
        query,
        ..read::<Projected<T>>(endpoint, summary)
    }
}

/// The derived reads: every projection, node detail, its explanations, and history.
fn projections() -> Vec<Operation> {
    use crate::endpoints as at;
    vec![
        projection::<Snapshot>(
            &at::SNAPSHOT,
            "The bounded agent snapshot, scoped to a subtree and depth, its nodes paged (I3).",
            query::SNAPSHOT_PARAMS,
        ),
        projection::<Level>(
            &at::LEVEL,
            "One canvas level: the kinds shown, at the top or within a container (C2).",
            query::LEVEL_PARAMS,
        ),
        Operation {
            path: &[JOURNEY_ID, NODE_KEY],
            ..projection::<Trace>(
                &at::TRACE,
                "What is upstream and downstream of a node, gravity contributors marked (C7).",
                &[],
            )
        },
        projection::<DecisionView>(&at::DECISIONS, "The decision view (C12).", &[]),
        projection::<Timeline>(&at::TIMELINE, "The timeline (C13).", &[]),
        projection::<StatusSummary>(&at::SUMMARY, "The status summary (C18).", &[]),
        projection::<Next>(
            &at::NEXT,
            "The acting frontier ranked, globally or for the caller, filtered (C10).",
            query::NEXT_PARAMS,
        ),
        projection::<ListPage>(
            &at::NODES,
            "The nodes a list query matches, sorted and paged (C9).",
            query::LIST_PARAMS,
        ),
        projection::<Mine>(
            &at::MINE,
            "The nodes the caller participates in, by participation kind (E4).",
            query::MINE_PARAMS,
        ),
    ]
}

/// Node detail, its explanation pages, and history.
fn node_reads() -> Vec<Operation> {
    use crate::endpoints as at;
    vec![
        Operation {
            path: &[JOURNEY_ID, NODE_KEY],
            ..projection::<NodeDetail>(
                &at::NODE,
                "One node in full; each explanation list carries its largest entries up to \
                 the response limit and its total (C8).",
                &[],
            )
        },
        Operation {
            path: &[
                JOURNEY_ID,
                NODE_KEY,
                (
                    "field",
                    "The value whose explanation list to page.",
                    schema_of::<ExplainedField>,
                ),
            ],
            ..projection::<ExplanationPage>(
                &at::EXPLANATIONS,
                "A page of a node's explanation list, largest first (C8).",
                query::EXPLANATION_PARAMS,
            )
        },
        Operation {
            path: &[JOURNEY_ID],
            query: query::HISTORY_PARAMS,
            ..read::<History>(
                &at::HISTORY,
                "The journey's events, or a node's, grouped by patch and paged (J4).",
            )
        },
    ]
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

/// A write to a proposal: a body of `T`, answered with what the proposal now is.
fn proposal_write<T: JsonSchema>(endpoint: &'static Endpoint, summary: &'static str) -> Operation {
    Operation {
        body: Some(schema_of::<T>),
        success: json::<ProposalAnswer>(),
        patch: true,
        ..operation(endpoint, summary)
    }
}

/// Proposals: created against a domain, then addressed by id (I6, C14).
fn proposals() -> Vec<Operation> {
    use crate::endpoints as at;
    let create = "Creates a proposal for this domain, which may not exist yet, under a \
                  client-generated id; a create resubmitted under that id answers the proposal \
                  (I6).";
    vec![
        Operation {
            path: &[JOURNEY_ID],
            ..proposal_write::<ProposalCreate>(&at::PROPOSE_JOURNEY, create)
        },
        Operation {
            path: &[ROUTE_ID],
            ..proposal_write::<ProposalCreate>(&at::PROPOSE_ROUTE, create)
        },
        proposal_write::<ProposalCreate>(&at::PROPOSE_DEPLOYMENT, create),
        Operation {
            path: &[PROPOSAL_ID],
            ..read::<Proposal>(
                &at::PROPOSAL,
                "A proposal: its destination, editing revision, status, and content (I6).",
            )
        },
        Operation {
            path: &[PROPOSAL_ID],
            ..proposal_write::<ProposalEdit>(
                &at::EDIT_PROPOSAL,
                "Replaces its content against its editing revision; its destination does not \
                 move (H5).",
            )
        },
        Operation {
            path: &[PROPOSAL_ID],
            ..read::<ProposalReview>(
                &at::PREVIEW_PROPOSAL,
                "What applying it now would do: unresolved items, violations, the graph and \
                 frontier after, consequences, and what moved since drafting (C14, D7, I6).",
            )
        },
        Operation {
            path: &[PROPOSAL_ID],
            body: Some(schema_of::<ProposalApply>),
            success: json::<PatchAnswer>(),
            patch: true,
            ..operation(
                &at::APPLY_PROPOSAL,
                "Applies it at the reviewed editing revision, the caller confirming (H2); \
                 stale with what intervened when either revision moved (I6).",
            )
        },
        Operation {
            path: &[PROPOSAL_ID],
            ..proposal_write::<ProposalStep>(&at::DISCARD_PROPOSAL, "Discards it (I6).")
        },
        Operation {
            path: &[PROPOSAL_ID],
            drafts: true,
            ..proposal_write::<ProposalStep>(
                &at::REFRESH_PROPOSAL,
                "Drafts it again on its destination as it stands, the reviewer's choices \
                 carried over; review it again before applying (I6).",
            )
        },
    ]
}

/// A proposal the engine drafts for a journey: a body of `T`.
fn drafted<T: JsonSchema>(endpoint: &'static Endpoint, summary: &'static str) -> Operation {
    Operation {
        path: &[JOURNEY_ID],
        drafts: true,
        ..proposal_write::<T>(endpoint, summary)
    }
}

/// Bulk and import: upgrade, save as route, and re-link as proposals; route files.
fn bulk() -> Vec<Operation> {
    use crate::endpoints as at;
    vec![
        drafted::<UpgradeRequest>(
            &at::UPGRADE,
            "Proposes upgrading the journey to a newer version of its route: the merge's \
             conflicts, kept edits, and orphans are its review items (B7).",
        ),
        drafted::<SaveAsRouteRequest>(
            &at::SAVE_AS_ROUTE,
            "Proposes saving the journey's structure as a route draft, with a participation \
             mapping per explicit entity and an exclusion per node (B8).",
        ),
        drafted::<RelinkRequest>(
            &at::RELINK,
            "Proposes re-linking the journey to a published version, differences kept as \
             local edits unless the reviewer takes the route's (B9).",
        ),
        Operation {
            path: &[ROUTE_ID],
            body: Some(schema_of::<RouteImport>),
            success: json::<PatchAnswer>(),
            patch: true,
            ..operation(
                &at::IMPORT_ROUTE,
                "Imports a route file as a new route or a new draft extending a published \
                 version, matched by key or path; one route patch (A13).",
            )
        },
        Operation {
            path: &[ROUTE_ID],
            query: query::EXPORT_PARAMS,
            ..read::<RouteFile>(
                &at::EXPORT_ROUTE,
                "A published version, or the draft, as a route file: every key kept, in one \
                 sorted order, naming the version it extends (A13).",
            )
        },
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

/// The problems an operation answers besides its success, by status.
fn problems_of(operation: &Operation) -> Vec<(&'static str, &'static str)> {
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
    if operation.query.iter().any(|spec| spec.name == "revision") {
        problems.push((
            "409",
            "The journey moved since the page the cursor came from: start again from the first \
             page.",
        ));
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
    problems
}

fn responses(
    operation: &Operation,
    generator: &mut SchemaGenerator,
    problem: &Schema,
    rejection: &Schema,
    tick: &Schema,
) -> Value {
    let json_of = |schema: &Value, description: &str| json!({"description": description, "content": {"application/json": {"schema": schema}}});
    let mut answers = Map::new();
    match operation.success {
        Success::Json(status, schema) => {
            answers.insert(
                status.to_string(),
                json_of(schema(generator).as_value(), "Answered."),
            );
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
    for (status, description) in problems_of(operation) {
        answers.insert(status.to_owned(), json_of(problem.as_value(), description));
    }
    auth_refusals(&mut answers);
    if operation.patch {
        let stale = "Stale (with what intervened) or a reused patch id (H5).";
        answers.insert("409".to_owned(), json_of(rejection.as_value(), stale));
        let invalid = "Invalid: every violation, by path (A15).";
        answers.insert("422".to_owned(), json_of(rejection.as_value(), invalid));
    }
    if operation.drafts {
        let either = json!({"oneOf": [rejection, problem]});
        let invalid = "Invalid: every violation, by path (A15); or a problem when it cannot be \
                       drafted (B7, B8, B9).";
        answers.insert("422".to_owned(), json_of(&either, invalid));
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
