//! The assistant and its turn loop (ARCHITECTURE, Assistant; PRD I5, I7, H2): a turn sends the
//! stored dialogue and the user's message with the MCP tool set's definitions to the
//! provider, runs every tool call it answers in process as the assistant acting for its user,
//! and repeats until the model answers without calling a tool, within the iteration,
//! provider call, and turn limits. Its writes are reported whatever ends it.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cairn_auth::Clock;
use cairn_mcp::{ToolDefinition, ToolError, ToolSet, Writes, instructions};
use cairn_schema::{Actor, AgentId, ChangeClass, ConversationId, Markdown};
use cairn_service::Service;
use cairn_store::{ConversationMessage, ConversationRecord, MessageAuthor, Store};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::Semaphore;
use tokio::time::Instant;

use crate::chat::{Chat, Completion, ProviderConfig, ProviderError, ToolCall, Usage};
use crate::conversation::{self, ConversationEntry, Target};
use crate::limits::{
    DIALOGUE_REPLAY_BYTES_MAX, PROVIDER_CALL_DURATION_MAX, TOOL_CALL_COUNT_PER_REPLY_MAX,
    TOOL_LOOP_ITERATION_COUNT_MAX, TOOL_RESULT_BYTES_PER_TURN_MAX, TURN_DURATION_MAX,
    TURN_IN_FLIGHT_COUNT_MAX, TURN_SAVE_RESERVE,
};
use crate::receipts::{Proposed, Receipts, Write};
use crate::tool_results::{self, Failures};

/// The counter of turns, by how they ended.
pub const TURNS_TOTAL: &str = "cairn_assistant_turns_total";

/// The counter of tokens the provider reported, by kind (input, cached, output).
pub const PROVIDER_TOKENS_TOTAL: &str = "cairn_assistant_provider_tokens_total";

/// The agent id every assistant write records beside the user it acts for (H2).
pub const ASSISTANT_AGENT: &str = "ag_assistant";

/// The assistant's agent id.
#[must_use]
pub fn assistant_agent() -> AgentId {
    match ASSISTANT_AGENT.parse() {
        Ok(agent) => agent,
        Err(error) => unreachable!("{ASSISTANT_AGENT} is an agent id: {error:?}"),
    }
}

/// What one turn answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TurnReply {
    /// The conversation the turn was added to: the user's, about its target.
    pub conversation: ConversationId,
    /// What the assistant said last, if anything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply: Option<Markdown>,
    /// What the turn added to the conversation, oldest first: the user's words, a line for each
    /// write, why the turn stopped if it did, and the reply. Given even when the conversation
    /// could not be saved.
    pub messages: Vec<ConversationEntry>,
    // PRD: I5, D7.
    /// Every write the assistant made to a journey, route, or the deployment, in order, with
    /// what it newly caused.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writes: Vec<Write>,
    /// Every proposal the assistant drafted, changed, or discarded, once each, for the user
    /// to review.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proposals: Vec<Proposed>,
    /// How the turn ended.
    pub ended: Ended,
}

/// How a turn ended. The writes before any ending stand and are reported.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Ended {
    /// The assistant answered.
    Replied,
    /// The turn stopped before an answer: the provider failed or is not set up, the reply
    /// was cut off or declined, the model kept repeating a failed call or never stopped
    /// calling tools, or the turn ran out of time.
    Stopped {
        /// Why, for the user.
        message: String,
    },
}

impl Ended {
    fn stopped(message: &str) -> Self {
        Ended::Stopped {
            message: message.to_owned(),
        }
    }
}

/// Why a turn did not run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnError {
    /// `assistant_turns_in_flight` turns are already running: try again shortly.
    Overloaded,
    /// A turn of this conversation is already running: try again once it has answered.
    ConversationBusy,
    /// The journey does not exist.
    TargetMissing(Target),
    /// Reading the target or the conversation ran past the turn's limit: nothing was
    /// written.
    TimedOut,
    /// The store or the server failed: nothing more than what is reported was written.
    Failed(String),
}

impl std::fmt::Display for TurnError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TurnError::Overloaded => write!(
                formatter,
                "{TURN_IN_FLIGHT_COUNT_MAX} assistant turns are in flight \
                 (assistant_turns_in_flight)"
            ),
            TurnError::ConversationBusy => formatter
                .write_str("a reply in this conversation is still in progress; wait for it"),
            TurnError::TargetMissing(target) => write!(formatter, "no {target}"),
            TurnError::TimedOut => formatter
                .write_str("reading the target or the conversation ran past the turn's limit"),
            TurnError::Failed(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for TurnError {}

/// The assistant (I5): the tool set over the service, the deployment's provider, and the
/// store its conversations live in. Optional: a host without one has none at its root.
#[derive(Clone)]
pub struct Assistant {
    tools: ToolSet,
    /// The tools the model may call: the tool set's, but for those that are the user's alone.
    offered: Arc<[ToolDefinition]>,
    service: Service,
    store: Arc<dyn Store>,
    chat: Arc<Chat>,
    clock: Clock,
    in_flight: Arc<Semaphore>,
    /// The conversations a turn is running in: one turn at a time each, so no turn's
    /// messages are lost to another's save.
    busy: Arc<Mutex<BTreeSet<ConversationId>>>,
}

impl Assistant {
    /// The assistant over `service` and `store` (the service's own), at `clock`'s now (the
    /// deployment clock the API reads), asking the provider `config` describes.
    #[must_use]
    pub fn new(
        service: Service,
        store: Arc<dyn Store>,
        clock: Clock,
        config: ProviderConfig,
    ) -> Self {
        // I7: applying a proposal is the user's click, and importing a file is the user's
        // too.
        let offered = ToolSet::definitions()
            .into_iter()
            .filter(|tool| match tool.writes {
                Writes::Nothing | Writes::Patch | Writes::Proposal => true,
                Writes::Confirmation | Writes::File => false,
            })
            .collect();
        Self {
            tools: ToolSet::new(service.clone(), clock.clone()),
            offered,
            service,
            store,
            chat: Arc::new(Chat::new(config)),
            clock,
            in_flight: Arc::new(Semaphore::new(TURN_IN_FLIGHT_COUNT_MAX as usize)),
            busy: Arc::default(),
        }
    }

    /// The conversation `actor`'s user holds about `target`, if they have started one.
    ///
    /// # Errors
    ///
    /// [`TurnError::Failed`] when the store fails.
    pub async fn conversation(
        &self,
        actor: &Actor,
        target: &Target,
    ) -> Result<Option<ConversationRecord>, TurnError> {
        let id = conversation::conversation_id(target, &actor.user);
        self.store
            .conversation(&id)
            .await
            .map_err(|error| TurnError::Failed(error.to_string()))
    }

    /// I5: one turn of `actor`'s conversation about `target`, starting from `message`;
    /// `actor` is a user, since the endpoints that call this are for users only. The
    /// turn runs on its own task, so a client that goes away never cuts a write off; its
    /// conversation is saved and its writes reported however it ends.
    ///
    /// # Errors
    ///
    /// [`TurnError`] when it cannot run: too many in flight, a journey that does not exist,
    /// or a failed store.
    pub async fn turn(
        &self,
        actor: &Actor,
        target: Target,
        message: Markdown,
    ) -> Result<TurnReply, TurnError> {
        let Ok(slot) = Arc::clone(&self.in_flight).try_acquire_owned() else {
            return Err(TurnError::Overloaded);
        };
        let id = conversation::conversation_id(&target, &actor.user);
        let Some(running) = Running::start(&self.busy, id) else {
            return Err(TurnError::ConversationBusy);
        };
        let this = self.clone();
        let user = actor.clone();
        let task = tokio::spawn(async move {
            let answer = this.run(&user, &target, &message).await;
            drop(running);
            drop(slot);
            answer
        });
        match task.await {
            Ok(answer) => answer,
            Err(error) if error.is_panic() => std::panic::resume_unwind(error.into_panic()),
            Err(error) => Err(TurnError::Failed(error.to_string())),
        }
    }

    /// The turn itself.
    async fn run(
        &self,
        user: &Actor,
        target: &Target,
        message: &Markdown,
    ) -> Result<TurnReply, TurnError> {
        let started = self.clock.now();
        // The turn's work ends a reserve before its limit, so its conversation is saved
        // within it; every wait is held to one or the other.
        let end = Instant::now() + TURN_DURATION_MAX;
        let deadline = end - TURN_SAVE_RESERVE;
        let exists = tokio::time::timeout_at(deadline, self.exists(target))
            .await
            .map_err(|_| TurnError::TimedOut)??;
        let failed = |error: cairn_store::StoreError| TurnError::Failed(error.to_string());
        let id = conversation::conversation_id(target, &user.user);
        let stored = tokio::time::timeout_at(deadline, self.store.conversation(&id)).await;
        let stored = stored.map_err(|_| TurnError::TimedOut)?.map_err(failed)?;
        let held = stored.is_some();
        let mut record =
            stored.unwrap_or_else(|| conversation::started(target, &user.user, started));
        let mut messages = vec![
            json!({ "role": "system", "content": system() }),
            json!({ "role": "user", "content": working_for(user, target) }),
        ];
        messages.extend(conversation::dialogue(&record, DIALOGUE_REPLAY_BYTES_MAX));
        messages.push(json!({ "role": "user", "content": message.as_str() }));
        let acting = Actor {
            user: user.user.clone(),
            agent: Some(assistant_agent()),
        };
        let mut receipts = Receipts::default();
        let (reply, ended) = self
            .converse(&acting, messages, deadline, &mut receipts)
            .await;

        let now = self.clock.now();
        let mut kept: Vec<ConversationMessage> = Vec::new();
        kept.extend(conversation::message(
            MessageAuthor::User,
            started,
            message.as_str(),
        ));
        kept.extend(
            receipts
                .lines(target)
                .iter()
                .filter_map(|line| conversation::message(MessageAuthor::Tool, now, line)),
        );
        if let Ended::Stopped { message } = &ended {
            let note = format!("The turn stopped: {message}");
            kept.extend(conversation::message(MessageAuthor::Tool, now, &note));
        }
        let reply =
            reply.and_then(|text| conversation::message(MessageAuthor::Assistant, now, &text));
        kept.extend(reply.clone());
        let messages: Vec<ConversationEntry> = kept.iter().cloned().map(Into::into).collect();
        conversation::append(&mut record, kept, now);
        // A route that does not exist yet keeps no conversation until a turn drafts one:
        // naming routes that do not exist stores nothing.
        let wrote = !receipts.is_empty();
        if exists || held || wrote {
            self.save(record, end, wrote).await?;
        }
        let label = match ended {
            Ended::Replied => "replied",
            Ended::Stopped { .. } => "stopped",
        };
        metrics::counter!(TURNS_TOTAL, "ended" => label).increment(1);
        tracing::info!(conversation = %id, ended = label, writes = receipts.len(), "assistant turn");
        let (writes, proposals) = receipts.into_parts();
        Ok(TurnReply {
            conversation: id,
            reply: reply.map(|message| message.content),
            messages,
            writes,
            proposals,
            ended,
        })
    }

    /// Whether `target` exists. A journey must, or the turn is refused; a route's draft may
    /// not yet, since its first proposal creates it (A12).
    async fn exists(&self, target: &Target) -> Result<bool, TurnError> {
        let missing = || TurnError::TargetMissing(target.clone());
        match target {
            Target::Journey(journey) => match self.service.journey(journey).await {
                Ok(Some(_)) => Ok(true),
                Ok(None) => Err(missing()),
                Err(error) => Err(TurnError::Failed(format!("{error:?}"))),
            },
            Target::RouteDraft(route) => match self.service.route(route).await {
                Ok(read) => Ok(read.is_some()),
                Err(error) => Err(TurnError::Failed(error.to_string())),
            },
        }
    }

    /// Saves `record` before `end`. A failed save fails the turn only when it `wrote`
    /// nothing; otherwise its writes stand and are answered (I5: every direct write is
    /// reported), and only the conversation is not kept.
    async fn save(
        &self,
        record: ConversationRecord,
        end: Instant,
        wrote: bool,
    ) -> Result<(), TurnError> {
        let id = record.id.clone();
        match tokio::time::timeout_at(end, self.store.put_conversation(record)).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) if !wrote => Err(TurnError::Failed(error.to_string())),
            Ok(Err(error)) => {
                tracing::warn!(conversation = %id, %error, "assistant conversation not saved");
                Ok(())
            }
            Err(_) => {
                tracing::warn!(conversation = %id, "assistant conversation not saved in time");
                Ok(())
            }
        }
    }

    /// The tool loop: provider calls until one answers without a tool call, or a limit.
    async fn converse(
        &self,
        acting: &Actor,
        mut messages: Vec<Value>,
        deadline: Instant,
        receipts: &mut Receipts,
    ) -> (Option<String>, Ended) {
        let tools: Vec<Value> = self.offered.iter().map(tool_json).collect();
        let mut failures = Failures::default();
        for _ in 0..TOOL_LOOP_ITERATION_COUNT_MAX {
            let reply = match self.ask(&messages, &tools, deadline).await {
                Ok(reply) => reply,
                Err(ended) => return (None, ended),
            };
            // A reply that did not finish runs none of its calls: they may be cut mid-way.
            if !reply.finished {
                let why = "the reply was cut off or declined, so nothing it asked for was done";
                return (reply.text, Ended::stopped(why));
            }
            if reply.calls.is_empty() {
                return (reply.text, Ended::Replied);
            }
            messages.push(reply.message());
            for (index, call) in reply.calls.iter().enumerate() {
                if Instant::now() >= deadline {
                    return (None, Ended::stopped(TURN_TIMED_OUT));
                }
                if index >= TOOL_CALL_COUNT_PER_REPLY_MAX as usize {
                    let refused = ToolError::Refused {
                        message: format!(
                            "at most {TOOL_CALL_COUNT_PER_REPLY_MAX} tool calls run per reply \
                             (assistant_tool_calls_per_reply)"
                        ),
                    };
                    messages.push(result(&call.id, Err(refused)));
                    continue;
                }
                let Some(tool) = self.offered.iter().find(|tool| tool.name == call.name) else {
                    let message = format!(
                        "{} is not a tool the assistant runs; some writes are the user's alone, \
                         such as applying a proposal: tell them it is ready for review",
                        call.name
                    );
                    messages.push(result(&call.id, Err(ToolError::Refused { message })));
                    if failures.failed(&call.name, &call.arguments) {
                        return (None, Ended::stopped(REPEATED_FAILURE));
                    }
                    continue;
                };
                let answer = match self.call_tool(acting, tool, call, deadline, receipts).await {
                    Ok(answer) => answer,
                    Err(ended) => return (None, ended),
                };
                let failed = answer.is_err();
                messages.push(result(&call.id, answer));
                if failed && failures.failed(&call.name, &call.arguments) {
                    return (None, Ended::stopped(REPEATED_FAILURE));
                }
            }
            tool_results::keep_newest(&mut messages, TOOL_RESULT_BYTES_PER_TURN_MAX);
        }
        patina_dst::sometimes!(true, "assistant-iteration-limit-reached");
        let why = "the assistant reached its tool-call limit";
        (None, Ended::stopped(why))
    }

    /// One tool call, on its own task so a commit the deadline overtakes still completes. Err
    /// when the deadline overtakes it: a write it lands within half the save reserve is
    /// reported with the turn (I5: every direct write is reported); one later still is not,
    /// which the turn's stop note says.
    async fn call_tool(
        &self,
        acting: &Actor,
        tool: &ToolDefinition,
        call: &ToolCall,
        deadline: Instant,
        receipts: &mut Receipts,
    ) -> Result<Result<Value, ToolError>, Ended> {
        let (tools, actor) = (self.tools.clone(), acting.clone());
        let (name, arguments) = (call.name.clone(), call.arguments.clone());
        let mut running = tokio::spawn(async move { tools.call(&actor, &name, arguments).await });
        let answer = match tokio::time::timeout_at(deadline, &mut running).await {
            Ok(Ok(answer)) => answer,
            Ok(Err(error)) if error.is_panic() => std::panic::resume_unwind(error.into_panic()),
            Ok(Err(_)) => return Err(Ended::stopped(TURN_TIMED_OUT)),
            Err(_) => {
                patina_dst::sometimes!(true, "assistant-tool-call-past-deadline");
                let late = deadline + TURN_SAVE_RESERVE / 2;
                if let Ok(Ok(Ok(output))) = tokio::time::timeout_at(late, running).await {
                    receipts.record(tool.writes, &call.name, &call.arguments, &output);
                }
                return Err(Ended::stopped(TURN_TIMED_OUT));
            }
        };
        if let Ok(output) = &answer {
            receipts.record(tool.writes, &call.name, &call.arguments, output);
        }
        Ok(answer)
    }

    /// One provider call, held to the provider call limit and what is left of the turn.
    async fn ask(
        &self,
        messages: &[Value],
        tools: &[Value],
        deadline: Instant,
    ) -> Result<Completion, Ended> {
        let left = deadline.saturating_duration_since(Instant::now());
        let limit = left.min(PROVIDER_CALL_DURATION_MAX);
        if limit == Duration::ZERO {
            return Err(Ended::stopped(TURN_TIMED_OUT));
        }
        // PRACTICES, Simulation: a provider that does not answer, which no runtime fault
        // reaches from outside the process.
        if patina_dst::buggify!("assistant-provider-timeout") {
            return Err(Ended::stopped(PROVIDER_TIMED_OUT));
        }
        let started = Instant::now();
        let answer = tokio::time::timeout(limit, self.chat.complete(messages, tools)).await;
        metrics::histogram!("cairn_assistant_provider_call_seconds")
            .record(started.elapsed().as_secs_f64());
        match answer {
            Ok(Ok(reply)) => {
                observe(&reply);
                Ok(reply)
            }
            Ok(Err(ProviderError::TimedOut)) => Err(Ended::stopped(PROVIDER_TIMED_OUT)),
            Ok(Err(ProviderError::Failed { message })) => {
                tracing::warn!(%message, "assistant provider call failed");
                Err(Ended::stopped("the model provider failed"))
            }
            Ok(Err(ProviderError::Rejected { status, message })) => {
                tracing::warn!(status, %message, "assistant provider call rejected");
                Err(Ended::Stopped {
                    message: if status == 401 {
                        "the assistant is not set up on this deployment: the model provider \
                         refused its key"
                            .to_owned()
                    } else {
                        format!("the model provider failed (status {status})")
                    },
                })
            }
            Err(_) if limit < PROVIDER_CALL_DURATION_MAX => Err(Ended::stopped(TURN_TIMED_OUT)),
            Err(_) => {
                patina_dst::sometimes!(true, "assistant-provider-call-timed-out");
                Err(Ended::stopped(PROVIDER_TIMED_OUT))
            }
        }
    }
}

const PROVIDER_TIMED_OUT: &str = "the model provider did not answer in time";
const REPEATED_FAILURE: &str = "the assistant kept repeating a tool call that failed";
const TURN_TIMED_OUT: &str = "the turn ran past its time limit; a change still being written \
                              then may land after it, so check the journey";

/// The system text: how writes land, then the guide every agent reads (I4, I5: the same
/// instructions as external agents), which points to `get_workflow` for the workflows. It
/// holds nothing that differs between users, targets or turns, so with the tool list it is
/// the one prefix every request shares and a provider can serve from its cache.
fn system() -> String {
    format!(
        "You are Cairn's in-app assistant. You have the same tools as any agent acting for \
         the user you work for, and every write you make is recorded as yours, on their \
         behalf.\n\n\
         How your writes land: a state change ({state}) applies directly, and its result \
         lists what it caused: tell the user. A structural change ({structural}) is made \
         only as a proposal (`create_proposal`): tell the user it is ready for review. A \
         route is never created or changed with `apply_patch`, not even to follow a workflow \
         step that says so: draft it as a proposal, also when the route does not exist yet. \
         Applying a proposal and importing a file are the user's alone.\n\n\
         # Guide\n\n{guide}\n",
        state = ChangeClass::State.meaning(),
        structural = ChangeClass::Structural.meaning(),
        guide = instructions::guide(),
    )
}

/// Who the conversation works for and on what: the first message after the system text, the
/// same on every turn of the conversation.
fn working_for(user: &Actor, target: &Target) -> String {
    format!("You are working for user {} on {target}.", user.user)
}

/// A tool as a chat-completions request lists it.
fn tool_json(tool: &ToolDefinition) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.input_schema,
        },
    })
}

/// A tool call's result as the model is sent it.
fn result(call_id: &str, answer: Result<Value, ToolError>) -> Value {
    let content = match answer {
        Ok(output) => output.to_string(),
        Err(error) => serde_json::to_string(&error).unwrap_or_else(|_| error.to_string()),
    };
    json!({ "role": "tool", "tool_call_id": call_id, "content": content })
}

/// Records what a provider call used and called: a metric per kind of token, and a log line.
fn observe(reply: &Completion) {
    let Usage {
        input,
        cached,
        output,
    } = reply.usage;
    for (kind, tokens) in [("input", input), ("cached", cached), ("output", output)] {
        metrics::counter!(PROVIDER_TOKENS_TOTAL, "kind" => kind).increment(tokens);
    }
    let tools: Vec<&str> = reply.calls.iter().map(|call| call.name.as_str()).collect();
    tracing::info!(input, cached, output, tools = %tools.join(","), "assistant provider call");
}

/// A conversation marked busy while its turn runs, unmarked when dropped, however the
/// turn ends.
struct Running {
    busy: Arc<Mutex<BTreeSet<ConversationId>>>,
    id: ConversationId,
}

impl Running {
    /// Marks `id` busy, unless a turn of it already is.
    fn start(busy: &Arc<Mutex<BTreeSet<ConversationId>>>, id: ConversationId) -> Option<Self> {
        let inserted = lock(busy).insert(id.clone());
        inserted.then(|| Self {
            busy: Arc::clone(busy),
            id,
        })
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        lock(&self.busy).remove(&self.id);
    }
}

/// The set behind `mutex`, whether or not a holder panicked: it is plain data.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
