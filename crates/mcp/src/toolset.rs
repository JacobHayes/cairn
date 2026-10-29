//! The tool set (ARCHITECTURE, MCP endpoint; Assistant): every tool's definition, and one
//! entry point that parses a call's arguments against the tool's schema, runs it as the
//! caller, and answers its output as JSON. The MCP server serves it over Streamable HTTP;
//! the assistant calls it in process, so an external agent and the assistant are the
//! same thing with a different transport (I7).

use cairn_auth::Clock;
use cairn_schema::{Actor, Patch, Rejection, TouchedSet};
use cairn_service::{Call, DomainPatch, Service};
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
    /// What it writes; a tool that writes nothing only reads.
    pub writes: Writes,
    /// The JSON Schema of its arguments, as listed to an agent: an object, compacted of
    /// what an agent cannot act on ([`crate::compact`]).
    pub input_schema: Map<String, Value>,
    /// The JSON Schema of what it answers when it succeeds: an object. Not listed over MCP
    /// (decisions/2026-10-06-the-tool-list-carries-the-mutation-vocabulary-once-and-no.md);
    /// the endpoint's tests hold every output to it.
    pub output_schema: Map<String, Value>,
}

// PRD: I5, I7.
/// What a tool writes: what the endpoint's annotations say of it, and what the assistant's
/// write wrapper decides its calls by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Writes {
    /// Nothing: it reads.
    Nothing,
    /// One patch it drafts from its arguments, which [`ToolSet::drafted_patch`] works out
    /// without writing.
    Patch,
    /// One patch Cairn builds from a file the caller hands it: what it writes is known only
    /// once it is written.
    File,
    /// A proposal, for someone to review: nothing reaches its destination until one is
    /// applied.
    Proposal,
    /// A reviewed proposal applied to its destination, the caller recorded as confirming it.
    Confirmation,
}

impl Writes {
    /// Whether it writes anything.
    #[must_use]
    pub const fn any(self) -> bool {
        !matches!(self, Writes::Nothing)
    }
}

/// A tool's entry in a module's table: its definition but for the schema, which is generated
/// from its arguments' type on demand.
pub(crate) struct Spec {
    pub name: &'static str,
    pub description: &'static str,
    pub writes: Writes,
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
/// vocabulary out once, so the tool list an agent loads carries it once
/// (decisions/2026-10-06-the-tool-list-carries-the-mutation-vocabulary-once-and-no.md).
/// Parsing is as strict either way.
pub(crate) fn schema_citing_mutations<T: JsonSchema>() -> Map<String, Value> {
    citing::<T>(&[MUTATION, REVIEW_ITEM])
}

/// The schema of `T`, `apply_patch`'s arguments: the one place the mutation vocabulary is
/// spelled out, as a type sketch generated from the mutation schema
/// ([`crate::sketch`]), of the ops `listed` keeps: exactly those the tool takes. The
/// `mutations` items are objects whose `op` is one of those op names, described by the
/// sketch; parsing is by the typed schema, as strict as ever.
pub(crate) fn schema_with_mutation_sketch<T: JsonSchema>(
    listed: &dyn Fn(&str) -> bool,
) -> Map<String, Value> {
    let mut schema = cited::<T>(&[REVIEW_ITEM]);
    let Some(Value::Object(definitions)) = schema.get_mut("$defs") else {
        unreachable!("a tool that takes mutations has shared definitions");
    };
    let Some(mutation) = definitions.get(MUTATION.0).cloned() else {
        unreachable!("a tool that takes mutations reaches the mutation definition");
    };
    // The schemas are fixed by the types at build time: a construct the sketch cannot
    // render is a bug the listing's tests reach, not a condition to answer.
    let crate::sketch::MutationSketch { text: sketch, ops } = crate::sketch::mutation_sketch(
        definitions,
        MUTATION.0,
        &mutation,
        (crate::sketch::GROUPS, listed),
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    definitions.insert(
        MUTATION.0.to_owned(),
        serde_json::json!({
            "type": "object",
            "required": ["op"],
            "properties": { "op": { "enum": ops } },
            "description": format!(
                "One mutation: an object with `op` and that op's fields, as typed here (`?` \
                 may be left out, `| null` takes null, `one of` takes exactly one):\n{sketch}"
            ),
        }),
    );
    prune_definitions(&mut schema);
    schema
}

/// A shared definition a schema cites rather than spells out: its name and what is said
/// in its place.
pub(crate) type Citation = (&'static str, &'static str);

const MUTATION: Citation = (
    "Mutation",
    "One mutation, as `apply_patch` defines them: an object whose `op` names it (`add_node`, \
     `answer`, `set_pin`, and the rest), with that mutation's fields.",
);

pub(crate) const REVIEW_ITEM: Citation = (
    "ReviewItem",
    "A review item, as `get_proposal` shows them: Cairn drafts them for an upgrade, a save \
     as route, or a re-link; a proposal you draft yourself needs none.",
);

/// The schema of `T` with each of `citations` described in a sentence, and what only they
/// reached dropped.
fn citing<T: JsonSchema>(citations: &[Citation]) -> Map<String, Value> {
    let mut schema = cited::<T>(citations);
    prune_definitions(&mut schema);
    schema
}

/// The schema of `T` with each of `citations` described in a sentence, nothing pruned yet.
fn cited<T: JsonSchema>(citations: &[Citation]) -> Map<String, Value> {
    let mut schema = schema::<T>();
    if let Some(Value::Object(definitions)) = schema.get_mut("$defs") {
        for (name, description) in citations {
            let cited = serde_json::json!({ "type": "object", "description": description });
            if definitions.contains_key(*name) {
                definitions.insert((*name).to_owned(), cited);
            }
        }
    }
    schema
}

/// Drops the `$defs` entries nothing outside them reaches any more.
pub(crate) fn prune_definitions(schema: &mut Map<String, Value>) {
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
#[derive(Clone)]
pub struct ToolSet {
    pub(crate) service: Service,
    clock: Clock,
}

impl ToolSet {
    /// The tools over `service`, each call made at `clock`'s now (the deployment clock the
    /// API's calls read).
    #[must_use]
    pub fn new(service: Service, clock: Clock) -> Self {
        Self { service, clock }
    }

    /// A call made now as `actor`.
    pub(crate) fn call_as(&self, actor: &Actor) -> Call {
        Call {
            actor: actor.clone(),
            now: self.clock.now(),
        }
    }

    /// Every tool, in the order they are listed to an agent.
    #[must_use]
    pub fn definitions() -> Vec<ToolDefinition> {
        tools::specs()
            .map(|spec| ToolDefinition {
                name: spec.name,
                description: spec.description,
                writes: spec.writes,
                input_schema: crate::compact::compact(
                    (spec.schema)(),
                    &[MUTATION.0, REVIEW_ITEM.0],
                ),
                output_schema: (spec.output)(),
            })
            .collect()
    }

    // PRD: H2, I7.
    /// Runs the tool `name` with `arguments` as `actor`: an agent acts as its user,
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
        let call = self.call_as(actor);
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

    // PRD: I5.
    /// What `patch` writes applied now as `actor`, at the current revisions, without
    /// writing it, or the rejection that would refuse it there: what the assistant's write
    /// wrapper counts the nodes of.
    ///
    /// # Errors
    ///
    /// [`ToolError::Failed`] when the read fails.
    pub async fn touched(
        &self,
        actor: &Actor,
        patch: &Patch,
    ) -> Result<Result<TouchedSet, Rejection>, ToolError> {
        Ok(self.service.touched(&self.call_as(actor), patch).await?)
    }

    // PRD: I5.
    /// The domain patch the direct-write tool `name` would submit for `arguments`, worked
    /// out as [`call`](Self::call) would, without writing anything: what the assistant's
    /// write wrapper reads to decide whether a write applies directly or becomes a proposal.
    /// `None` for a tool that drafts no patch from its arguments (any but [`Writes::Patch`]).
    ///
    /// # Errors
    ///
    /// [`ToolError::Arguments`] when the arguments do not match the tool's schema; what
    /// drafting reads can fail with otherwise.
    pub async fn drafted_patch(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<Option<DomainPatch>, ToolError> {
        match tools::spec(name).map(|spec| spec.writes) {
            Some(Writes::Patch) => self.drafted_patch_of(name, arguments).await,
            Some(Writes::Nothing | Writes::File | Writes::Proposal | Writes::Confirmation)
            | None => Ok(None),
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
