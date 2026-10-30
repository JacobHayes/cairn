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

use crate::endpoints::{self, Endpoint};
use crate::query::{self, ParamSpec, schema_of};
use crate::wire::{
    AgentToken, AssistantRequest, Conversation, MintedToken, TokenRequest, TurnReply,
};
use cairn_schema::{
    AgentId, DecisionView, Deployment, Derived, DomainDocument, ExplainedField, ExplanationPage,
    IdentityKey, InsertionAsk, InsertionPlan, Journey, JourneyId, Level, ListPage, Next, NodeKey,
    Proposal, ProposalId, Rejection, RemovalPlan, RemovalRead, RemovalTarget, Route, RouteFile,
    RouteId, RouteVersion, Snapshot, StatusSummary, Timeline, Trace, VersionNumber,
};
use cairn_service::wire::{
    Caller, Capabilities, EventPage, Health, History, IdentityReferences, JourneyPage, Mine,
    NodeDetail, PatchAnswer, PatchRequest, PatchSubmitted, Problem, Projected, ProposalAnswer,
    ProposalApply, ProposalEdit, ProposalList, ProposalReview, ProposalStep, RouteDetail,
    RouteImport, RoutePage, SearchPage, Tick, Tour, TourWrite, UserName,
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
    /// A conflict answers 409 with this body, in these words.
    conflict: Option<(&'static str, SchemaFn)>,
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
        conflict: None,
        drafts: false,
    }
}

const JOURNEY_ID: (&str, &str, SchemaFn) = ("id", "The journey.", schema_of::<JourneyId>);
const ROUTE_ID: (&str, &str, SchemaFn) = ("id", "The route.", schema_of::<RouteId>);
const PROPOSAL_ID: (&str, &str, SchemaFn) = (
    "proposal",
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
        success: json::<PatchSubmitted>(),
        patch: true,
        drafts: true,
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
    let mut all = vec![health()];
    all.extend(writes());
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
    all.extend(assistant());
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
            "The agent overview: the journey's facts and counts; the nodes are `listNodes`.",
            &[],
        ),
        projection::<Level>(
            &at::LEVEL,
            "One canvas level: the kinds shown, at the top or within a container.",
            query::LEVEL_PARAMS,
        ),
        Operation {
            path: &[JOURNEY_ID, NODE_KEY],
            ..projection::<Trace>(
                &at::TRACE,
                "What is upstream and downstream of a node, gravity contributors marked.",
                &[],
            )
        },
        projection::<Derived>(
            &at::DERIVED,
            "Every derived value with its explanation inputs, as the browser derives the document.",
            &[],
        ),
        projection::<DecisionView>(&at::DECISIONS, "The decision view.", &[]),
        projection::<Timeline>(&at::TIMELINE, "The timeline.", &[]),
        projection::<StatusSummary>(&at::SUMMARY, "The status summary.", &[]),
        projection::<Next>(
            &at::NEXT,
            "The acting frontier ranked, globally or for the caller, filtered.",
            query::NEXT_PARAMS,
        ),
        projection::<ListPage>(
            &at::NODES,
            "The nodes a list query matches, sorted and paged.",
            query::LIST_PARAMS,
        ),
        projection::<Mine>(
            &at::MINE,
            "The nodes the caller participates in, by participation kind.",
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
                 the response limit and its total.",
                &[],
            )
        },
        Operation {
            path: &[JOURNEY_ID, NODE_KEY],
            ..projection::<RemovalRead>(
                &at::NODE_REMOVAL,
                "The exact removal of a node as the journey stands, and the references \
                 elsewhere that the same patch must rewrite or remove, a page at a time.",
                query::EXPLANATION_PARAMS,
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
                "A page of a node's explanation list, largest first.",
                query::EXPLANATION_PARAMS,
            )
        },
        Operation {
            path: &[JOURNEY_ID],
            query: query::HISTORY_PARAMS,
            ..read::<History>(
                &at::HISTORY,
                "The journey's events, or a node's, grouped by patch and paged.",
            )
        },
    ]
}

/// `info.version`: the document describes the API at the commit it is generated from, which has
/// no version of its own; releases are named by tags
/// (decisions/2026-10-16-the-openapi-document-is-unversioned.md).
const DOCUMENT_VERSION: &str = "unversioned";

/// The health check: public, so a proxy's anonymous probe can ask it.
fn health() -> Operation {
    read::<Health>(
        &crate::endpoints::HEALTH,
        "Whether the server is serving: 200 while its store answers, 503 once the store has \
         failed closed. Needs no credential.",
    )
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
                "Applies a journey patch; base revision 0 creates it.",
            )
        },
        Operation {
            path: &[ROUTE_ID],
            ..patch(
                &at::PATCH_ROUTE,
                "Applies a route patch, its draft included.",
            )
        },
        patch(
            &at::PATCH_DEPLOYMENT,
            "Applies a deployment patch: identities and merges.",
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

/// Proposals: created against a domain, then listed and addressed under it (I6, C14).
fn proposals() -> Vec<Operation> {
    use crate::endpoints as at;
    let mut all = Vec::new();
    all.extend(proposals_under(
        &at::JOURNEY_PROPOSALS,
        &[JOURNEY_ID],
        &[JOURNEY_ID, PROPOSAL_ID],
    ));
    all.extend(proposals_under(
        &at::ROUTE_PROPOSALS,
        &[ROUTE_ID],
        &[ROUTE_ID, PROPOSAL_ID],
    ));
    all.extend(proposals_under(
        &at::DEPLOYMENT_PROPOSALS,
        &[],
        &[PROPOSAL_ID],
    ));
    all.push(Operation {
        query: query::PROPOSAL_SCOPE_PARAMS,
        ..read::<ProposalList>(
            &at::PROPOSALS,
            "The open proposals no one destination holds: those waiting on the caller, and those \
             that would create a journey or a route, newest first with their count.",
        )
    });
    all
}

/// The operations on the proposals under one destination (I6, C14): `at` is the path
/// parameters of the destination's list, `under` of one proposal.
fn proposals_under(
    set: &'static endpoints::ProposalEndpoints,
    at: &'static [(&'static str, &'static str, SchemaFn)],
    under: &'static [(&'static str, &'static str, SchemaFn)],
) -> Vec<Operation> {
    vec![
        Operation {
            path: at,
            query: query::PROPOSAL_PARAMS,
            ..read::<ProposalList>(
                &set.list,
                "The destination's open proposals, newest first, with their count and the \
                 destination as it is now.",
            )
        },
        Operation {
            path: under,
            ..read::<Proposal>(
                &set.get,
                "A proposal under this destination: its editing revision, status, and content.",
            )
        },
        Operation {
            path: under,
            ..proposal_write::<ProposalEdit>(
                &set.edit,
                "Replaces its content against its editing revision; its destination does not \
                 move.",
            )
        },
        Operation {
            path: under,
            ..read::<ProposalReview>(
                &set.preview,
                "What applying it now would do: unresolved items, violations, the graph and \
                 frontier after, consequences, and what moved since drafting.",
            )
        },
        Operation {
            path: under,
            body: Some(schema_of::<ProposalApply>),
            success: json::<PatchAnswer>(),
            patch: true,
            ..operation(
                &set.apply,
                "Applies it at the reviewed editing revision, the caller confirming; \
                 stale with what intervened when either revision moved.",
            )
        },
        Operation {
            path: under,
            ..proposal_write::<ProposalStep>(&set.discard, "Discards it.")
        },
        Operation {
            path: under,
            drafts: true,
            ..proposal_write::<ProposalStep>(
                &set.refresh,
                "Drafts it again on its destination as it stands, the reviewer's choices \
                 carried over; review it again before applying.",
            )
        },
    ]
}

/// Bulk and import: route files.
fn bulk() -> Vec<Operation> {
    use crate::endpoints as at;
    vec![
        Operation {
            path: &[ROUTE_ID],
            body: Some(schema_of::<RouteImport>),
            success: json::<PatchAnswer>(),
            patch: true,
            ..operation(
                &at::IMPORT_ROUTE,
                "Imports a route file as a new route or a new draft extending a published \
                 version, matched by key or path; one route patch.",
            )
        },
        Operation {
            path: &[ROUTE_ID],
            query: query::EXPORT_PARAMS,
            ..read::<RouteFile>(
                &at::EXPORT_ROUTE,
                "A published version, or the draft, as a route file: every key kept, in one \
                 sorted order, naming the version it extends.",
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
            ..read::<JourneyPage>(&at::JOURNEYS, "The journey index, filtered and paged.")
        },
        Operation {
            path: &[JOURNEY_ID],
            ..read::<Journey>(&at::JOURNEY, "A journey with its graph and state.")
        },
        Operation {
            path: &[JOURNEY_ID],
            ..read::<DomainDocument>(
                &at::DOCUMENT,
                "The domain document: the journey and the caller's derive inputs; nothing \
                 derived.",
            )
        },
        Operation {
            query: query::ROUTE_PARAMS,
            ..read::<RoutePage>(&at::ROUTES, "The route index, paged.")
        },
        Operation {
            path: &[ROUTE_ID],
            ..read::<Route>(&at::ROUTE, "A route with its draft.")
        },
        Operation {
            path: &[ROUTE_ID],
            ..read::<RouteDetail>(
                &at::ROUTE_VERSIONS,
                "A route's versions and their journeys.",
            )
        },
        Operation {
            path: &[
                ROUTE_ID,
                ("version", "The version.", schema_of::<VersionNumber>),
            ],
            ..read::<RouteVersion>(&at::ROUTE_VERSION, "One published route version.")
        },
        read::<Deployment>(&at::DEPLOYMENT, "The deployment: identities and aliases."),
        Operation {
            path: &[("key", "The identity.", schema_of::<IdentityKey>)],
            ..read::<IdentityReferences>(
                &at::IDENTITY_REFERENCES,
                "The journeys and routes referring to an identity, directly or through an \
                 alias: what a delete is checked against.",
            )
        },
        Operation {
            query: query::SEARCH_PARAMS,
            ..read::<SearchPage>(&at::SEARCH, "Text search across journeys, paged.")
        },
        Operation {
            query: query::EVENT_PARAMS,
            ..read::<EventPage>(&at::EVENTS, "Events, filtered and paged in commit order.")
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
                "Revision ticks as server-sent `tick` events, current revisions first.",
            )
        },
        read::<Caller>(&at::CALLER, "The caller and their identities."),
        Operation {
            query: query::USER_NAME_PARAMS,
            ..read::<Vec<UserName>>(
                &at::USER_NAMES,
                "The names people see for the users asked for: the identity their verified emails \
                 name, else the name their sign-in gave.",
            )
        },
        read::<Vec<AgentToken>>(&at::TOKENS, "The caller's agent tokens."),
        Operation {
            body: Some(schema_of::<TokenRequest>),
            success: Success::Json(201, schema_of::<MintedToken>),
            ..operation(&at::MINT_TOKEN, "Mints an agent token, shown this once.")
        },
        Operation {
            path: &[("agent", "The token's agent.", schema_of::<AgentId>)],
            ..operation(
                &at::REVOKE_TOKEN,
                "Revokes one of the caller's agent tokens.",
            )
        },
        read::<Tour>(
            &at::TOUR_GET,
            "The caller's place in the tour of Cairn, and today in the deployment's zone.",
        ),
        Operation {
            body: Some(schema_of::<TourWrite>),
            conflict: Some((
                "The caller's record is not the one the write expected: the current record, \
                 unchanged.",
                schema_of::<Tour>,
            )),
            ..operation(
                &at::TOUR_PUT,
                "Moves the caller's place in the tour of Cairn, only over the record the write \
                 expects.",
            )
        },
    ]
}

/// The assistant's turns (I5), served when the capabilities offer the assistant.
fn assistant() -> Vec<Operation> {
    use crate::endpoints as at;
    let turn = |endpoint, path, summary| Operation {
        path,
        body: Some(schema_of::<AssistantRequest>),
        success: json::<TurnReply>(),
        ..operation(endpoint, summary)
    };
    vec![
        turn(
            &at::ASSISTANT_JOURNEY,
            &[JOURNEY_ID],
            "One assistant turn about a journey, when the capabilities offer the assistant \
: its direct writes with their consequences, and its proposals for review.",
        ),
        turn(
            &at::ASSISTANT_ROUTE_DRAFT,
            &[ROUTE_ID],
            "One assistant turn about a route's draft, when the capabilities offer the \
             assistant: what it drafts arrives as proposals on the draft.",
        ),
        Operation {
            path: &[JOURNEY_ID],
            success: json::<Conversation>(),
            ..operation(
                &at::ASSISTANT_JOURNEY_CONVERSATION,
                "The caller's conversation with the assistant about a journey, as kept, when \
                 the capabilities offer the assistant.",
            )
        },
        Operation {
            path: &[ROUTE_ID],
            success: json::<Conversation>(),
            ..operation(
                &at::ASSISTANT_ROUTE_DRAFT_CONVERSATION,
                "The caller's conversation with the assistant about a route's draft, as \
                 kept, when the capabilities offer the assistant.",
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
        .with(|settings| {
            settings.definitions_path = "/components/schemas".into();
            settings
                .transforms
                .push(Box::new(cairn_schema::json_schema::without_sketch_marks()));
        })
        .into_generator();
    let problem = generator.subschema_for::<Problem>();
    let rejection = generator.subschema_for::<Rejection>();
    let tick = generator.subschema_for::<Tick>();
    // Types only the browser's engine answers, so no endpoint names them: the app's types.
    generator.subschema_for::<RemovalTarget>();
    generator.subschema_for::<RemovalPlan>();
    generator.subschema_for::<InsertionAsk>();
    generator.subschema_for::<InsertionPlan>();
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
            "version": DOCUMENT_VERSION,
            "description": "The Cairn HTTP API. Every request but the health check is authenticated (a session cookie or a bearer token). A rejected patch answers its rejection unchanged, every violation by path; other errors answer a Problem with the request id.",
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
    if operation.endpoint.public {
        // No credential: the document's default security does not apply.
        rendered["security"] = json!([]);
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
        let mut array = json!({"type": "array", "items": one});
        if let Some(count_max) = spec.count_max {
            array["maxItems"] = json!(count_max);
        }
        array
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
    let mut problems = Vec::new();
    if !operation.endpoint.public {
        problems.push(("400", "The request is malformed."));
    }
    problems.extend([
        (
            "500",
            "The server failed; the request id names it in the logs.",
        ),
        (
            "503",
            "A limit was reached or the request timed out; retry after `Retry-After`.",
        ),
    ]);
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
    if operation.endpoint.user_only {
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
    if operation.endpoint.public {
        // The health check's own 503 carries its body; a limit's carries a problem.
        let health = generator.subschema_for::<Health>();
        let either = json!({"oneOf": [health, problem]});
        let unavailable = "The store has failed closed (a Health), or a limit was reached \
                           (a problem).";
        answers.insert("503".to_owned(), json_of(&either, unavailable));
    } else {
        auth_refusals(&mut answers);
    }
    if operation.patch {
        let stale = "Stale (with what intervened) or a reused patch id.";
        answers.insert("409".to_owned(), json_of(rejection.as_value(), stale));
        let invalid = "Invalid: every violation, by path.";
        answers.insert("422".to_owned(), json_of(rejection.as_value(), invalid));
    }
    if let Some((description, schema)) = operation.conflict {
        answers.insert(
            "409".to_owned(),
            json_of(schema(generator).as_value(), description),
        );
    }
    if operation.drafts {
        let either = json!({"oneOf": [rejection, problem]});
        let invalid = "Invalid: every violation, by path; or a problem when it cannot be \
                       drafted.";
        answers.insert("422".to_owned(), json_of(&either, invalid));
    }
    Value::Object(answers)
}

/// The auth layer's own answers, which stand in front of every operation in plain text
/// (3.2): 401 with its `WWW-Authenticate` challenge, 403 for a peer a local-only provider
/// or a proxy-mode provider refuses, and 503 when a sign-in provider cannot be asked (no `Retry-After`). A status
/// the API also answers as a problem gets the text body beside it.
fn auth_refusals(answers: &mut Map<String, Value>) {
    let refusals = [
        (
            "401",
            "No credential, or one that is refused (the auth layer, in text).",
        ),
        (
            "403",
            "The auth layer refused the peer: a local-only provider and a remote peer, or \
             login headers from a peer no proxy-mode provider trusts (in text).",
        ),
        (
            "503",
            "The auth layer could not ask a sign-in provider (in text, without `Retry-After`).",
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
