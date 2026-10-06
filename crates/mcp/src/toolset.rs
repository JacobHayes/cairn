//! The tool set (ARCHITECTURE, MCP endpoint; Assistant): every tool's definition, and one
//! entry point that parses a call's arguments against the tool's schema, runs it as the
//! caller, and answers its output as JSON. The MCP server serves it over Streamable HTTP;
//! the assistant (4.4) calls it in process, so an external agent and the assistant are the
//! same thing with a different transport (I7).

use cairn_auth::Clock;
use cairn_schema::Actor;
use cairn_service::{Call, Service};
use cairn_store::Store;
use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::ToolError;
use crate::tools;

/// One tool as an agent sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolDefinition {
    /// Its name.
    pub name: &'static str,
    /// What it does and when to use it, for the agent.
    pub description: &'static str,
    /// Whether it writes (a patch or a proposal); a tool that does not only reads.
    pub writes: bool,
    /// The JSON Schema of its arguments: an object.
    pub input_schema: Map<String, Value>,
    /// The JSON Schema of what it answers when it succeeds: an object. Not listed over MCP
    /// (DECISIONS.md, 4.3); the endpoint's tests hold every output to it.
    pub output_schema: Map<String, Value>,
}

/// A tool's entry in a module's table: its definition but for the schema, which is generated
/// from its arguments' type on demand.
pub(crate) struct Spec {
    pub name: &'static str,
    pub description: &'static str,
    pub writes: bool,
    /// A write that may undo or overwrite what is there (a skip, a clear, a discard, a
    /// merge, any patch), not only add to it: the MCP `destructiveHint`.
    pub destructive: bool,
    pub schema: fn() -> Map<String, Value>,
    pub output: fn() -> Map<String, Value>,
}

/// The schema of `T`, a tool's arguments or output, as a self-contained object schema (its
/// shared types under `$defs`).
pub(crate) fn schema<T: JsonSchema>() -> Map<String, Value> {
    let generator = SchemaSettings::draft2020_12().into_generator();
    let schema = generator.into_root_schema_for::<T>();
    match schema.to_value() {
        Value::Object(object) => object,
        other => unreachable!("an arguments type's schema is an object, not {other}"),
    }
}

/// The schema of `T`, a tool's arguments that carry mutations, with each mutation described
/// as an object whose `op` names it rather than spelled out: `apply_patch` spells the
/// vocabulary out once, so the tool list an agent loads carries it once (DECISIONS.md, 4.3).
/// Parsing is as strict either way.
pub(crate) fn schema_citing_mutations<T: JsonSchema>() -> Map<String, Value> {
    citing::<T>(&[MUTATION, REVIEW_ITEM])
}

/// The schema of `T`, a tool's arguments that carry mutations but no proposal content: its
/// review items, which only a proposal holds and Cairn drafts, are cited.
pub(crate) fn schema_citing_review_items<T: JsonSchema>() -> Map<String, Value> {
    citing::<T>(&[REVIEW_ITEM])
}

/// A shared definition a schema cites rather than spells out: its name and what is said
/// in its place.
type Citation = (&'static str, &'static str);

const MUTATION: Citation = (
    "Mutation",
    "One mutation, as `apply_patch` defines them: an object whose `op` names it (`add_node`, \
     `answer`, `set_pin`, and the rest), with that mutation's fields.",
);

const REVIEW_ITEM: Citation = (
    "ReviewItem",
    "A review item, as `get_proposal` shows them: Cairn drafts them for an upgrade, a save \
     as route, or a re-link; a proposal you draft yourself needs none.",
);

/// The schema of `T` with each of `citations` described in a sentence, and what only they
/// reached dropped.
fn citing<T: JsonSchema>(citations: &[Citation]) -> Map<String, Value> {
    let mut schema = schema::<T>();
    if let Some(Value::Object(definitions)) = schema.get_mut("$defs") {
        for (name, description) in citations {
            let cited = serde_json::json!({ "type": "object", "description": description });
            if definitions.contains_key(*name) {
                definitions.insert((*name).to_owned(), cited);
            }
        }
    }
    prune_definitions(&mut schema);
    schema
}

/// Drops the `$defs` entries nothing outside them reaches any more.
fn prune_definitions(schema: &mut Map<String, Value>) {
    let Some(Value::Object(definitions)) = schema.remove("$defs") else {
        return;
    };
    let mut reached = std::collections::BTreeSet::new();
    let mut pending: Vec<&Value> = schema.values().collect();
    while let Some(value) = pending.pop() {
        match value {
            Value::Object(object) => {
                let target = object.get("$ref").and_then(Value::as_str);
                let name = target.and_then(|target| target.strip_prefix("#/$defs/"));
                if let Some(name) = name
                    && reached.insert(name.to_owned())
                    && let Some(definition) = definitions.get(name)
                {
                    pending.push(definition);
                }
                pending.extend(object.values());
            }
            Value::Array(items) => pending.extend(items),
            _ => {}
        }
    }
    let kept: Map<String, Value> = definitions
        .into_iter()
        .filter(|(name, _)| reached.contains(name))
        .collect();
    schema.insert("$defs".to_owned(), Value::Object(kept));
}

/// The tool set over one service: what the MCP endpoint serves and the assistant calls.
pub struct ToolSet<S> {
    pub(crate) service: Service<S>,
    clock: Clock,
}

impl<S> Clone for ToolSet<S> {
    fn clone(&self) -> Self {
        Self {
            service: self.service.clone(),
            clock: self.clock.clone(),
        }
    }
}

impl<S: Store + 'static> ToolSet<S> {
    /// The tools over `service`, each call made at `clock`'s now (the deployment clock the
    /// API's calls read).
    #[must_use]
    pub fn new(service: Service<S>, clock: Clock) -> Self {
        Self { service, clock }
    }

    /// Every tool, in the order they are listed to an agent.
    #[must_use]
    pub fn definitions() -> Vec<ToolDefinition> {
        tools::specs()
            .map(|spec| ToolDefinition {
                name: spec.name,
                description: spec.description,
                writes: spec.writes,
                input_schema: (spec.schema)(),
                output_schema: (spec.output)(),
            })
            .collect()
    }

    /// Runs the tool `name` with `arguments` as `actor` (H2, I7): an agent acts as its user,
    /// with the user's capabilities, and its writes record both.
    ///
    /// # Errors
    ///
    /// [`ToolError::UnknownTool`] for a name no tool has; [`ToolError::Arguments`] when the
    /// arguments do not match its schema; otherwise what the tool answers.
    pub async fn call(
        &self,
        actor: &Actor,
        name: &str,
        arguments: Value,
    ) -> Result<Value, ToolError> {
        let call = Call {
            actor: actor.clone(),
            now: self.clock.now(),
        };
        match tools::group_of(name) {
            Some(tools::Group::Derive) => self.derive(&call, name, arguments).await,
            Some(tools::Group::Read) => self.read(&call, name, arguments).await,
            Some(tools::Group::Write) => self.write(&call, name, arguments).await,
            Some(tools::Group::Propose) => self.propose(&call, name, arguments).await,
            None => Err(ToolError::UnknownTool {
                name: name.to_owned(),
            }),
        }
    }
}

/// Parses a tool's arguments, naming the path of the first value that does not match.
pub(crate) fn parse<T: DeserializeOwned>(arguments: Value) -> Result<T, ToolError> {
    serde_path_to_error::deserialize(arguments).map_err(|error| ToolError::Arguments {
        path: match error.path().to_string() {
            root if root == "." => String::new(),
            path => path,
        },
        message: error.into_inner().to_string(),
    })
}

/// A tool's output as JSON.
pub(crate) fn output<T: Serialize>(answer: Result<T, ToolError>) -> Result<Value, ToolError> {
    let value = answer?;
    serde_json::to_value(value).map_err(|error| ToolError::Failed {
        message: format!("the output did not serialize: {error}"),
    })
}
