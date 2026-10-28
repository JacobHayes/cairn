//! The assistant and its turn loop (ARCHITECTURE, Assistant; PRD I5, I7, H2): a turn reads
//! its target fresh, sends the conversation with the tool set's definitions to the provider,
//! runs every tool call it answers through the write wrapper as the assistant acting for its
//! user, and repeats until the model answers without calling a tool, within the iteration,
//! provider call, and turn limits. Its writes are reported whatever ends it.

use std::collections::BTreeSet;
use std::fmt::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cairn_auth::Clock;
use cairn_mcp::{ToolError, ToolSet, instructions};
use cairn_schema::{Actor, AgentId, ChangeClass, ConversationId, Markdown};
use cairn_service::Service;
use cairn_store::{ConversationMessage, ConversationRecord, MessageAuthor, Store};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::Semaphore;
use tokio::time::Instant;

use crate::conversation::{self, Target};
use crate::limits::{
    DIALOGUE_REPLAY_BYTES_MAX, PROVIDER_CALL_DURATION_MAX, TOOL_CALL_COUNT_PER_REPLY_MAX,
    TOOL_LOOP_ITERATION_COUNT_MAX, TOOL_RESULT_BYTES_PER_TURN_MAX, TURN_DURATION_MAX,
    TURN_IN_FLIGHT_COUNT_MAX, TURN_SAVE_RESERVE,
};
use crate::provider::{
    Exchange, Message, Provider, ProviderError, Reply, Stop, ToolResult, ToolSpec,
};
use crate::tool_results::{self, Failures};
use crate::wrapper::{self, Action};

/// The counter of turns, by how they ended and the status class of a refused provider call.
pub const TURNS_TOTAL: &str = "cairn_assistant_turns_total";

/// The counter of tokens the provider reported, by kind (input, output, cache read, cache
/// write).
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
    // PRD: I5.
    /// Every write the assistant made, in order: applied with its consequences, or drafted
    /// as a proposal for review.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<Action>,
    /// How the turn ended.
    pub ended: Ended,
}

/// How a turn ended. The writes before any ending stand and are reported.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Ended {
    /// The assistant answered.
    Replied,
    /// The provider did not answer in time.
    ProviderTimedOut,
    /// The provider answered with an error.
    ProviderFailed {
        /// What it said.
        message: String,
    },
    /// The provider refused the deployment's credential: the assistant is not set up.
    ProviderNotSetUp,
    /// The reply hit the output limit and is cut off; none of its tool calls ran.
    ReplyCutOff,
    /// The model declined to answer.
    ModelRefused,
    /// The provider paused the reply, which the assistant does not continue; none of its
    /// tool calls ran.
    ModelPaused,
    /// The model kept repeating a tool call that failed.
    RepeatedFailure,
    /// The model was still calling tools past the tool-loop limit.
    IterationLimit,
    /// The turn ran past the turn's time limit.
    TurnTimedOut,
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
    store: Arc<dyn Store>,
    provider: Arc<dyn Provider>,
    clock: Clock,
    in_flight: Arc<Semaphore>,
    /// The conversations a turn is running in: one turn at a time each, so no turn's
    /// messages are lost to another's save.
    busy: Arc<Mutex<BTreeSet<ConversationId>>>,
}

impl Assistant {
    /// The assistant over `service` and `store` (the service's own), at `clock`'s now (the
    /// deployment clock the API reads), asking `provider`.
    #[must_use]
    pub fn new(
        service: Service,
        store: Arc<dyn Store>,
        clock: Clock,
        provider: Arc<dyn Provider>,
    ) -> Self {
        Self {
            tools: ToolSet::new(service, clock.clone()),
            store,
            provider,
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
    ) -> Result<Option<cairn_store::ConversationRecord>, TurnError> {
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
        // within it; every wait is held to one or the other (review 4.4 r2).
        let end = Instant::now() + TURN_DURATION_MAX;
        let deadline = end - TURN_SAVE_RESERVE;
        let read = tokio::time::timeout_at(deadline, self.context(user, target)).await;
        let (context, exists) = read.map_err(|_| TurnError::TimedOut)??;
        let failed = |error: cairn_store::StoreError| TurnError::Failed(error.to_string());
        let id = conversation::conversation_id(target, &user.user);
        let stored = tokio::time::timeout_at(deadline, self.store.conversation(&id)).await;
        let stored = stored.map_err(|_| TurnError::TimedOut)?.map_err(failed)?;
        let held = stored.is_some();
        let mut record =
            stored.unwrap_or_else(|| conversation::started(target, &user.user, started));
        let messages = replay(&record, message, &turn_context(user, target, &context));
        let exchange = Exchange {
            system: system(),
            messages,
            tools: tool_specs(),
        };
        let acting = Actor {
            user: user.user.clone(),
            agent: Some(assistant_agent()),
        };
        let mut provider_status = None;
        let (reply, actions, ended) = self
            .converse(&acting, exchange, deadline, &mut provider_status)
            .await;

        let now = self.clock.now();
        let mut kept: Vec<ConversationMessage> = Vec::new();
        kept.extend(conversation::message(
            MessageAuthor::User,
            started,
            message.as_str(),
        ));
        kept.extend(
            actions.iter().filter_map(|action| {
                conversation::message(MessageAuthor::Tool, now, &report(action))
            }),
        );
        if let Some(note) = ending_note(&ended) {
            kept.extend(conversation::message(MessageAuthor::Tool, now, &note));
        }
        let reply =
            reply.and_then(|text| conversation::message(MessageAuthor::Assistant, now, &text));
        kept.extend(reply.clone());
        conversation::append(&mut record, kept, now);
        // A route that does not exist yet keeps no conversation until a turn drafts one:
        // naming routes that do not exist stores nothing.
        if exists || held || !actions.is_empty() {
            self.save(record, end, !actions.is_empty()).await?;
        }
        let status_class = status_class(provider_status);
        metrics::counter!(
            TURNS_TOTAL,
            "ended" => ended_label(&ended),
            "status_class" => status_class
        )
        .increment(1);
        tracing::info!(conversation = %id, ended = ended_label(&ended), status_class, writes = actions.len(), "assistant turn");
        Ok(TurnReply {
            conversation: id,
            reply: reply.map(|message| message.content),
            actions,
            ended,
        })
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
    /// `provider_status` is set to the HTTP status of a call the provider refused.
    async fn converse(
        &self,
        acting: &Actor,
        mut exchange: Exchange,
        deadline: Instant,
        provider_status: &mut Option<u16>,
    ) -> (Option<String>, Vec<Action>, Ended) {
        let mut actions: Vec<Action> = Vec::new();
        let mut failures = Failures::default();
        for _ in 0..TOOL_LOOP_ITERATION_COUNT_MAX {
            let reply = match self.ask(&exchange, deadline).await {
                Ok(reply) => reply,
                Err((ended, status)) => {
                    *provider_status = status;
                    return (None, actions, ended);
                }
            };
            // A reply that did not finish runs none of its calls: they may be cut mid-way.
            if let Some(ended) = unfinished(reply.stop) {
                return (reply.text, actions, ended);
            }
            if reply.calls.is_empty() {
                return (reply.text, actions, Ended::Replied);
            }
            let mut results = Vec::with_capacity(reply.calls.len());
            for (index, call) in reply.calls.iter().enumerate() {
                if Instant::now() >= deadline {
                    return (None, actions, Ended::TurnTimedOut);
                }
                if index >= TOOL_CALL_COUNT_PER_REPLY_MAX as usize {
                    let refused = ToolError::Refused {
                        message: format!(
                            "at most {TOOL_CALL_COUNT_PER_REPLY_MAX} tool calls run per reply \
                             (assistant_tool_calls_per_reply)"
                        ),
                    };
                    results.push(result(&call.id, Err(refused)));
                    continue;
                }
                // On its own task, so a commit the deadline overtakes still completes.
                let (tools, store, actor) =
                    (self.tools.clone(), Arc::clone(&self.store), acting.clone());
                let (name, arguments) = (call.name.clone(), call.arguments.clone());
                let repeat = (name.clone(), arguments.clone());
                let mut running = tokio::spawn(async move {
                    let called = (name.as_str(), arguments);
                    wrapper::run(&tools, store.as_ref(), &actor, called).await
                });
                let outcome = match tokio::time::timeout_at(deadline, &mut running).await {
                    Ok(Ok(ran)) => ran,
                    Ok(Err(error)) if error.is_panic() => {
                        std::panic::resume_unwind(error.into_panic())
                    }
                    Ok(Err(_)) => return (None, actions, Ended::TurnTimedOut),
                    Err(_) => {
                        // The deadline overtook the call, which still runs: a write it lands
                        // within half the save reserve is reported with the turn (I5: every
                        // direct write is reported); one later still is not, which the
                        // turn's ending note says.
                        patina_dst::sometimes!(true, "assistant-tool-call-past-deadline");
                        let late = deadline + TURN_SAVE_RESERVE / 2;
                        if let Ok(Ok(outcome)) = tokio::time::timeout_at(late, running).await {
                            record(&mut actions, outcome.action);
                        }
                        return (None, actions, Ended::TurnTimedOut);
                    }
                };
                record(&mut actions, outcome.action);
                let failed = outcome.answer.is_err();
                results.push(result(&call.id, outcome.answer));
                if failed && failures.failed(&repeat.0, &repeat.1) {
                    return (None, actions, Ended::RepeatedFailure);
                }
            }
            exchange.messages.push(Message::Assistant(reply));
            exchange.messages.push(Message::ToolResults(results));
            tool_results::keep_newest(&mut exchange.messages, TOOL_RESULT_BYTES_PER_TURN_MAX);
        }
        patina_dst::sometimes!(true, "assistant-iteration-limit-reached");
        (None, actions, Ended::IterationLimit)
    }

    /// One provider call, held to the provider call limit and what is left of the turn; its
    /// failure also answers the HTTP status the provider refused it with.
    async fn ask(
        &self,
        exchange: &Exchange,
        deadline: Instant,
    ) -> Result<Reply, (Ended, Option<u16>)> {
        let left = deadline.saturating_duration_since(Instant::now());
        let limit = left.min(PROVIDER_CALL_DURATION_MAX);
        if limit == Duration::ZERO {
            return Err((Ended::TurnTimedOut, None));
        }
        // PRACTICES, Simulation: a provider that does not answer, which no runtime fault
        // reaches from outside the process.
        if patina_dst::buggify!("assistant-provider-timeout") {
            return Err((Ended::ProviderTimedOut, None));
        }
        let started = Instant::now();
        let answer = tokio::time::timeout(limit, self.provider.send(exchange)).await;
        metrics::histogram!("cairn_assistant_provider_call_seconds")
            .record(started.elapsed().as_secs_f64());
        match answer {
            Ok(Ok(reply)) => {
                observe(&reply);
                Ok(reply)
            }
            Ok(Err(ProviderError::TimedOut)) => Err((Ended::ProviderTimedOut, None)),
            Ok(Err(ProviderError::Failed { message })) => {
                tracing::warn!(%message, "assistant provider call failed");
                Err((Ended::ProviderFailed { message }, None))
            }
            Ok(Err(ProviderError::Rejected { status, message })) => {
                tracing::warn!(status, %message, "assistant provider call rejected");
                let ended = if status == 401 {
                    Ended::ProviderNotSetUp
                } else {
                    Ended::ProviderFailed {
                        message: format!("status {status}: {message}"),
                    }
                };
                Err((ended, Some(status)))
            }
            Err(_) if limit < PROVIDER_CALL_DURATION_MAX => Err((Ended::TurnTimedOut, None)),
            Err(_) => {
                patina_dst::sometimes!(true, "assistant-provider-call-timed-out");
                Err((Ended::ProviderTimedOut, None))
            }
        }
    }

    /// The turn's context (I5: the assistant reads its target): the journey's snapshot (I3),
    /// or the route's draft, read as the user.
    async fn context(&self, user: &Actor, target: &Target) -> Result<(Value, bool), TurnError> {
        let read = match target {
            Target::Journey(journey) => {
                self.tools
                    .call(user, "get_snapshot", json!({ "journey": journey }))
                    .await
            }
            Target::RouteDraft(route) => {
                self.tools
                    .call(user, "get_route", json!({ "route": route }))
                    .await
            }
        };
        match (read, target) {
            (Ok(context), _) => Ok((context, true)),
            (Err(ToolError::NotFound { .. }), Target::Journey(_)) => {
                Err(TurnError::TargetMissing(target.clone()))
            }
            // A route authored in conversation may not exist yet, or have no draft open: its
            // first proposal creates them (A12).
            (Err(ToolError::NotFound { message }), Target::RouteDraft(_)) => {
                Ok((json!({ "not_found": message }), false))
            }
            (Err(error), _) => Err(TurnError::Failed(error.to_string())),
        }
    }
}

/// The conversation as a provider is sent it: the stored dialogue, then `message` led by the
/// turn's `context`, so the request's prefix (tools, system text, history) is the same on
/// every request.
fn replay(record: &ConversationRecord, message: &Markdown, context: &str) -> Vec<Message> {
    let mut messages = conversation::dialogue(record, DIALOGUE_REPLAY_BYTES_MAX);
    let said = message.as_str();
    match messages.last_mut() {
        Some(Message::User { text }) => {
            text.push_str("\n\n");
            text.push_str(said);
            text.insert_str(0, context);
        }
        _ => messages.push(Message::User {
            text: format!("{context}{said}"),
        }),
    }
    messages
}

/// The tools the model is offered: every tool but those the wrapper always refuses.
fn tool_specs() -> Vec<ToolSpec> {
    ToolSet::definitions()
        .into_iter()
        .filter(|definition| wrapper::refusal(definition.writes).is_none())
        .map(|definition| ToolSpec {
            name: definition.name.to_owned(),
            description: definition.description.to_owned(),
            parameters: definition.input_schema,
        })
        .collect()
}

/// The system prompt, the same on every request (so a provider can cache it): the write
/// policy, the shipped guide, and the workflows by name and purpose, each read with
/// `get_workflow` when its job comes up (I4, I5: the same instructions as external agents).
/// Who the assistant works for and on what, and the target as read, are [`turn_context`],
/// which goes after it.
fn system() -> String {
    let mut refused: Vec<&str> = Vec::new();
    for definition in ToolSet::definitions() {
        if let Some(why) = wrapper::refusal(definition.writes)
            && !refused.contains(&why)
        {
            refused.push(why);
        }
    }
    let mut prompt = format!(
        "You are Cairn's in-app assistant. You have the same tools as any agent acting for \
         the user you are working for, and every write you make is recorded as yours, on \
         their behalf.\n\n\
         How your writes land: a state change ({state}) applies directly, like any external \
         agent's, and its result lists what it caused: tell the user. A structural change \
         ({structural}) is drafted as one proposal instead, which the result says: tell the \
         user it is ready for review. Some writes are the user's alone, and you have no tool for them:\n",
        state = ChangeClass::State.meaning(),
        structural = ChangeClass::Structural.meaning(),
    );
    for why in refused {
        let _ = writeln!(prompt, "- {why}");
    }
    let _ = write!(
        prompt,
        "\n# Guide\n\n{guide}\n\n# Workflows\n\nBefore a job one of these covers, read \
         it with get_workflow:\n\n",
        guide = instructions::guide()
    );
    for workflow in instructions::workflows() {
        let _ = writeln!(prompt, "- {}: {}", workflow.name, workflow.description);
    }
    prompt
}

/// What is true of this turn alone: who the assistant works for and on what, and the
/// target as read at the start of the turn. It leads the turn's user message.
fn turn_context(user: &Actor, target: &Target, context: &Value) -> String {
    format!(
        "# Context\n\nYou are working for user {user} on {target}, as read at the start of \
         this turn:\n\n```json\n{context}\n```\n\n# Message\n\n",
        user = user.user,
    )
}

/// A tool call's result as the model is sent it.
fn result(call_id: &str, answer: Result<Value, ToolError>) -> ToolResult {
    let (content, is_error) = match answer {
        Ok(output) => (output.to_string(), false),
        Err(error) => (
            serde_json::to_string(&error).unwrap_or_else(|_| error.to_string()),
            true,
        ),
    };
    ToolResult {
        call_id: call_id.to_owned(),
        content,
        is_error,
    }
}

/// Cairn's report of a write, kept in the conversation.
fn report(action: &Action) -> String {
    match action {
        Action::Applied { report, .. } => report.clone(),
        Action::AlreadyApplied { tool, receipt } => format!(
            "`{tool}` as patch `{}` was already applied: {} is at revision {}.",
            receipt.patch_id, receipt.domain, receipt.revision
        ),
        Action::Proposed {
            tool,
            proposal,
            because,
            ..
        } => {
            let why = match because {
                wrapper::Because::Structural => "it changes structure".to_owned(),
                wrapper::Because::Asked => "the assistant drafted a proposal".to_owned(),
            };
            format!("Drafted `{tool}` as proposal `{proposal}` for review: {why}.")
        }
        Action::Discarded { tool, proposal } => {
            format!("Discarded proposal `{proposal}` with `{tool}`.")
        }
    }
}

/// Cairn's note on a turn that ended without a reply.
fn ending_note(ended: &Ended) -> Option<String> {
    match ended {
        Ended::Replied => None,
        Ended::ProviderTimedOut => {
            Some("The turn ended: the model provider did not answer in time.".to_owned())
        }
        // The provider's own words are left out: this note is replayed to the model as the
        // assistant's, and an error text is not something it said.
        Ended::ProviderFailed { .. } => {
            Some("The turn ended: the model provider failed.".to_owned())
        }
        Ended::ProviderNotSetUp => {
            Some("The turn ended: the assistant is not set up on this deployment.".to_owned())
        }
        Ended::ReplyCutOff => {
            Some("The turn ended: the reply was cut off at the output limit.".to_owned())
        }
        Ended::ModelRefused => Some("The turn ended: the model declined to answer.".to_owned()),
        Ended::ModelPaused => {
            Some("The turn ended: the model paused before finishing its reply.".to_owned())
        }
        Ended::RepeatedFailure => {
            Some("The turn ended: the assistant kept repeating a tool call that failed.".to_owned())
        }
        Ended::IterationLimit => {
            Some("The turn ended: the assistant reached its tool-call limit.".to_owned())
        }
        Ended::TurnTimedOut => Some(
            "The turn ended: it ran past its time limit. A change still being written then may \
             land after this report; check the journey."
                .to_owned(),
        ),
    }
}

/// The metric and log label of an ending.
fn ended_label(ended: &Ended) -> &'static str {
    match ended {
        Ended::Replied => "replied",
        Ended::ProviderTimedOut => "provider_timed_out",
        Ended::ProviderFailed { .. } => "provider_failed",
        Ended::ProviderNotSetUp => "provider_not_set_up",
        Ended::ReplyCutOff => "reply_cut_off",
        Ended::ModelRefused => "model_refused",
        Ended::ModelPaused => "model_paused",
        Ended::RepeatedFailure => "repeated_failure",
        Ended::IterationLimit => "iteration_limit",
        Ended::TurnTimedOut => "turn_timed_out",
    }
}

/// The status class of a refused provider call, for the ended metric: `none` when the turn
/// did not end on one.
fn status_class(status: Option<u16>) -> &'static str {
    match status {
        Some(400..=499) => "4xx",
        Some(500..=599) => "5xx",
        Some(_) => "other",
        None => "none",
    }
}

/// How a turn ends on a reply that did not finish; `None` for one that did.
fn unfinished(stop: Stop) -> Option<Ended> {
    match stop {
        Stop::Done => None,
        Stop::OutputLimit => Some(Ended::ReplyCutOff),
        Stop::Refused => Some(Ended::ModelRefused),
        Stop::Paused => Some(Ended::ModelPaused),
    }
}

/// Records what a provider call used and called: a metric per kind of token, and a log line.
fn observe(reply: &Reply) {
    let usage = reply.usage;
    for (kind, tokens) in [
        ("input", usage.input),
        ("output", usage.output),
        ("cache_read", usage.cache_read),
        ("cache_write", usage.cache_write),
    ] {
        metrics::counter!(PROVIDER_TOKENS_TOTAL, "kind" => kind).increment(tokens);
    }
    let tools: Vec<&str> = reply.calls.iter().map(|call| call.name.as_str()).collect();
    tracing::info!(
        input = usage.input,
        output = usage.output,
        cache_read = usage.cache_read,
        cache_write = usage.cache_write,
        stop = ?reply.stop,
        tools = %tools.join(","),
        "assistant provider call"
    );
}

/// Adds `action` to the turn's: a later write to a proposal the turn already reported
/// replaces that report, so each proposal is reported once, as it stands.
fn record(actions: &mut Vec<Action>, action: Option<Action>) {
    let Some(action) = action else { return };
    let same = action.proposal().and_then(|proposal| {
        actions
            .iter()
            .position(|held| held.proposal() == Some(proposal))
    });
    match same {
        Some(position) => actions[position] = action,
        None => actions.push(action),
    }
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
