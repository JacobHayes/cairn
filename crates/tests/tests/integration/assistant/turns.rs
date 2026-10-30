//! Turns (I5, I7, H2, A12, B10; PRD Success criteria): a scripted model on a loopback server
//! drives the tool loop over the memory store, and each turn reports its writes, keeps its
//! conversation, and ends within its limits.
#![cfg(test)]

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use cairn_assistant::limits::{
    DIALOGUE_REPLAY_BYTES_MAX, TOOL_CALL_COUNT_PER_REPLY_MAX, TOOL_CALL_FAILURE_REPEAT_COUNT_MAX,
    TOOL_LOOP_ITERATION_COUNT_MAX, TOOL_RESULT_BYTES_PER_TURN_MAX, TURN_DURATION_MAX,
    TURN_IN_FLIGHT_COUNT_MAX, TURN_SAVE_RESERVE,
};
use cairn_assistant::tool_results::DROPPED;
use cairn_assistant::{Ended, Target, TurnError, TurnReply};
use cairn_mcp::{ToolSet, Writes, instructions};
use cairn_schema::Domain;
use cairn_schema::limits::BODY_BYTES_MAX;
use cairn_store::{ConversationStore, MessageAuthor};
use serde_json::{Value, json};

use crate::assistant::support::{
    KEY, Step, World, proposed_mutations, scenario, user, vendor_target,
};

const BAKEOFF: &str = "j_bakeoff";
const VENDOR: &str = "j_vendor_eval";

fn bakeoff() -> Target {
    Target::Journey(BAKEOFF.parse().unwrap())
}

fn stopped(turn: &TurnReply) -> &str {
    match &turn.ended {
        Ended::Stopped { message } => message,
        Ended::Replied => panic!("the turn replied: {turn:?}"),
    }
}

/// The journey's last patch, from its history.
async fn last_patch(world: &World, journey: &str) -> Value {
    let history = world
        .ok(
            &user("u_reader"),
            "get_history",
            json!({ "journey": journey }),
        )
        .await;
    history["patches"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone()
}

/// The roles and texts of the messages one request carried, in order.
fn roles(request: &Value) -> Vec<(String, String)> {
    request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|message| {
            let text = message["content"].as_str().unwrap_or_default();
            (
                message["role"].as_str().unwrap().to_owned(),
                text.to_owned(),
            )
        })
        .collect()
}

/// The `tool` messages a request carried, as JSON.
fn results(request: &Value) -> Vec<Value> {
    request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["role"] == "tool")
        .map(|message| serde_json::from_str(message["content"].as_str().unwrap()).unwrap())
        .collect()
}

fn complete_kickoff(patch: &str, base: u64) -> Value {
    json!({ "journey": VENDOR, "patch_id": patch, "base_revision": base,
        "mutations": [{ "op": "transition", "node": "n_kickoff", "transition": "complete" }] })
}

/// I7: the model is offered exactly the MCP tools but those that are the user's alone, with
/// the guide every agent reads as its instructions, and the user's message as it was said.
#[tokio::test]
async fn the_model_is_offered_the_mcp_tools_and_the_guide() {
    let world = World::new().await;
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    world.script([Step::say("Hello.")]);
    world.turn(&lead, &vendor_target(), "What is next?").await;
    let request = &world.provider.requests()[0];
    let offered: BTreeSet<&str> = request["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["function"]["name"].as_str().unwrap())
        .collect();
    let expected: Vec<_> = ToolSet::definitions()
        .into_iter()
        .filter(|tool| !matches!(tool.writes, Writes::Confirmation | Writes::File))
        .collect();
    assert_eq!(
        offered,
        expected
            .iter()
            .map(|tool| tool.name)
            .collect::<BTreeSet<_>>()
    );
    for tool in &expected {
        let sent = request["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|sent| sent["function"]["name"] == tool.name)
            .unwrap();
        assert_eq!(sent["function"]["parameters"], json!(tool.input_schema));
    }
    let sent = roles(request);
    assert_eq!(sent[0].0, "system");
    assert!(sent[0].1.contains(instructions::guide()));
    assert_eq!(sent.last().unwrap().1, "What is next?");
}

/// What a provider can serve from its cache: the system text and the tool list are the same
/// for every user and every target, so the first message that differs comes after them.
#[tokio::test]
async fn the_system_text_and_the_tools_do_not_depend_on_who_asks_or_where() {
    let world = World::new().await;
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    world.empty_bakeoff().await;
    world.script([Step::say("One."), Step::say("Two.")]);
    world.turn(&lead, &vendor_target(), "Hello?").await;
    world.turn(&user("u_owner"), &bakeoff(), "Hello?").await;
    let requests = world.provider.requests();
    assert_eq!(requests[0]["tools"], requests[1]["tools"]);
    assert_eq!(requests[0]["messages"][0], requests[1]["messages"][0]);
    assert_ne!(requests[0]["messages"][1], requests[1]["messages"][1]);
}

/// I7: applying a proposal and importing a file are the user's alone: named anyway, they are
/// refused and change nothing.
#[tokio::test]
async fn a_tool_outside_the_offered_list_is_refused_when_named() {
    let world = World::new().await;
    let owner = user("u_owner");
    world.empty_bakeoff().await;
    let structure = proposed_mutations(&scenario("bake-off"), 1);
    let create = json!({ "proposal": "pr_structure", "target": { "journey": BAKEOFF },
        "base_revision": 1, "note": "Structure", "mutations": structure, "patch_id": "p_structure" });
    let apply =
        json!({ "proposal": "pr_structure", "reviewed_revision": 1, "patch_id": "p_apply" });
    let import = json!({ "patch_id": "p_import", "file": "route: x\nname: X\nnodes: []\n" });
    world.script([
        Step::call("apply_patch", create),
        Step::calls([("apply_proposal", apply), ("import_route", import)]),
        Step::say("Ready for your review."),
    ]);
    let turn = world
        .turn(&owner, &bakeoff(), "Set it up and apply it.")
        .await;
    assert_eq!(turn.ended, Ended::Replied);
    assert_eq!(
        world.journey_revision(BAKEOFF).await,
        1,
        "nothing was applied"
    );
    let refusals = results(&world.provider.requests()[2]);
    assert_eq!(refusals.len(), 3);
    assert_eq!(refusals[1]["error"], "refused");
    assert_eq!(refusals[2]["error"], "refused");
}

/// PRACTICES, Explicit limits: a tool the assistant may not run, named again and again, ends
/// the turn like any other call that keeps failing.
#[tokio::test]
async fn a_forbidden_tool_named_again_ends_the_turn() {
    let world = World::new().await;
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    let apply = json!({ "proposal": "pr_x", "reviewed_revision": 1, "patch_id": "p_apply" });
    world.script(
        (0..=TOOL_CALL_FAILURE_REPEAT_COUNT_MAX)
            .map(|_| Step::call("apply_proposal", apply.clone())),
    );
    let turn = world.turn(&lead, &vendor_target(), "Apply it.").await;
    stopped(&turn);
    assert_eq!(
        world.provider.request_count(),
        TOOL_CALL_FAILURE_REPEAT_COUNT_MAX as usize
    );
}

/// The PRD's success criterion with no route (I5, I7, H2): the owner describes the work; the
/// assistant drafts the structure as a proposal while the journey stays where it was; the
/// owner applies it, and the events record the assistant as proposing and the owner as
/// confirming.
#[tokio::test]
async fn an_empty_journey_is_structured_through_a_proposal_the_user_applies() {
    let world = World::new().await;
    world.empty_bakeoff().await;
    let owner = user("u_owner");
    let structure = proposed_mutations(&scenario("bake-off"), 1);
    let create = json!({ "proposal": "pr_structure", "target": { "journey": BAKEOFF },
        "base_revision": 1, "note": "Structure the bake-off", "mutations": structure,
        "patch_id": "p_structure" });
    world.script([
        Step::call("get_snapshot", json!({ "journey": BAKEOFF })),
        Step::call("apply_patch", create),
        Step::say("I drafted the bake-off's structure as a proposal for you to review."),
    ]);
    let turn = world
        .turn(
            &owner,
            &bakeoff(),
            "Two-week bake-off between two options; set it up.",
        )
        .await;
    assert_eq!(turn.ended, Ended::Replied);
    assert!(turn.reply.is_some());
    assert_eq!(turn.writes, []);
    let [proposed] = turn.proposals.as_slice() else {
        panic!("{:?}", turn.proposals);
    };
    assert_eq!(
        proposed.destination,
        Domain::Journey(BAKEOFF.parse().unwrap())
    );
    assert!(!proposed.discarded);
    assert_eq!(
        world.journey_revision(BAKEOFF).await,
        1,
        "drafting moves nothing"
    );

    let apply = json!({ "proposal": proposed.proposal, "reviewed_revision": 1,
        "patch_id": "p_apply" });
    world.ok(&owner, "apply_proposal", apply).await;
    assert_eq!(world.journey_revision(BAKEOFF).await, 2);
    let patch = last_patch(&world, BAKEOFF).await;
    let events = patch["events"].as_array().unwrap();
    assert_ne!(events.as_slice(), [] as [Value; 0]);
    assert!(
        events
            .iter()
            .all(|event| event["confirming_user"] == "u_owner")
    );
    let held = world
        .ok(
            &owner,
            "get_proposal",
            json!({ "proposal": proposed.proposal }),
        )
        .await;
    assert_eq!(held["proposal"]["proposing_agent"], "ag_assistant");
    assert_eq!(held["proposal"]["created_by"], "u_owner");
}

/// I5, D7, H2: a state change applies directly and is answered with its revision and what it
/// caused; its events record the assistant and the user it acts for, and the conversation
/// keeps a plain line for it.
#[tokio::test]
async fn a_direct_write_is_reported_and_recorded_as_the_assistant_for_its_user() {
    let world = World::new().await;
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    let mut answers = crate::assistant::support::step_mutations(&scenario("vendor-evaluation"), 1);
    // A meeting far sooner than the work feeding it can be done.
    for answer in &mut answers {
        if answer["decision"] == "n_meeting_date" {
            answer["value"] = json!({ "date": "2026-10-06" });
        }
    }
    let patch = json!({ "patch_id": "p_answers", "target": { "journey": VENDOR },
        "base_revision": 1, "deployment_revision": 1, "mutations": answers });
    world.script([Step::call("apply_patch", patch), Step::say("Answered.")]);
    let turn = world
        .turn(
            &lead,
            &vendor_target(),
            "Answer the up-front decisions; meeting on the 6th.",
        )
        .await;
    assert_eq!(turn.ended, Ended::Replied);
    let [write] = turn.writes.as_slice() else {
        panic!("{:?}", turn.writes);
    };
    assert_eq!(write.tool, "apply_patch");
    assert_eq!(write.patch_id.as_str(), "p_answers");
    assert_eq!(write.domain, Domain::Journey(VENDOR.parse().unwrap()));
    assert_eq!(write.revision.get(), 2);
    let caused = &write.consequences[&VENDOR.parse().unwrap()];
    assert_ne!(
        caused.overdue,
        [],
        "the meeting leaves the earlier work overdue"
    );
    assert_eq!(
        last_patch(&world, VENDOR).await["actor"],
        json!({ "user": "u_lead", "agent": "ag_assistant" })
    );
    let held = world
        .store
        .conversation(&turn.conversation)
        .await
        .unwrap()
        .unwrap();
    let authors: Vec<MessageAuthor> = held.messages.iter().map(|message| message.author).collect();
    assert_eq!(
        authors,
        [
            MessageAuthor::User,
            MessageAuthor::Tool,
            MessageAuthor::Assistant
        ]
    );
    let line = held.messages[1].content.as_str();
    assert!(
        !line.contains("p_answers") && !line.contains(VENDOR),
        "a kept line names no id: {line}"
    );
}

/// I5: a write is reported even when a later provider call fails, and the conversation keeps
/// it and how the turn stopped.
#[tokio::test]
async fn a_write_stands_and_is_reported_when_the_provider_fails_after_it() {
    let world = World::new().await;
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    world.script([
        Step::call("update_journey_state", complete_kickoff("p_kickoff", 1)),
        Step::status(503),
    ]);
    let turn = world
        .turn(&lead, &vendor_target(), "Kickoff happened.")
        .await;
    assert!(stopped(&turn).contains("503"));
    assert_eq!(turn.writes.len(), 1);
    assert_eq!(world.journey_revision(VENDOR).await, 2);
    let held = world
        .store
        .conversation(&turn.conversation)
        .await
        .unwrap()
        .unwrap();
    let authors: Vec<MessageAuthor> = held.messages.iter().map(|message| message.author).collect();
    assert_eq!(
        authors,
        [
            MessageAuthor::User,
            MessageAuthor::Tool,
            MessageAuthor::Tool
        ],
        "the user's words, the write, and how the turn stopped"
    );
}

/// H5: a patch id applied before is reported as already applied, not as a new write.
#[tokio::test]
async fn a_write_applied_before_is_reported_as_already_applied() {
    let world = World::new().await;
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    let patch = complete_kickoff("p_kickoff", 1);
    world.script([
        Step::calls([
            ("update_journey_state", patch.clone()),
            ("update_journey_state", patch),
        ]),
        Step::say("Done."),
    ]);
    let turn = world
        .turn(&lead, &vendor_target(), "Kickoff happened.")
        .await;
    let flags: Vec<bool> = turn
        .writes
        .iter()
        .map(|write| write.already_applied)
        .collect();
    assert_eq!(flags, [false, true]);
    assert_eq!(world.journey_revision(VENDOR).await, 2);
}

/// A12: naming a route that does not exist stores no conversation until a turn drafts one;
/// a proposal the turn drafts and then discards is reported once, as discarded.
#[tokio::test]
async fn a_missing_route_keeps_no_conversation_and_a_discard_is_reported_as_one() {
    let world = World::new().await;
    world.script([Step::say("There is nothing here yet.")]);
    let target = Target::RouteDraft("nowhere".parse().unwrap());
    let turn = world.turn(&user("u_author"), &target, "Hello?").await;
    assert_eq!(
        world.store.conversation(&turn.conversation).await.unwrap(),
        None
    );

    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    let add = json!({ "op": "add_node", "node": { "key": "n_extra", "id": "extra",
        "kind": "action", "title": "An extra step" } });
    let create = json!({ "proposal": "pr_extra", "target": { "journey": VENDOR },
        "base_revision": 1, "note": "Add a step", "mutations": [add], "patch_id": "p_extra" });
    let discard = json!({ "proposal": "pr_extra", "change": "discard", "base_revision": 1,
        "patch_id": "p_discard_extra" });
    world.script([
        Step::call("apply_patch", create),
        Step::calls([
            ("edit_proposal", discard.clone()),
            ("edit_proposal", discard),
        ]),
        Step::say("Drafted, then dropped."),
    ]);
    let turn = world
        .turn(&lead, &vendor_target(), "Never mind the extra step.")
        .await;
    let [proposed] = turn.proposals.as_slice() else {
        panic!(
            "one report for the proposal, as it stands: {:?}",
            turn.proposals
        );
    };
    assert!(
        proposed.discarded,
        "a discard repeated from its receipt stays discarded"
    );
    // The turn answers what it added to the conversation, saved or not.
    let [said, _receipt, reply] = turn.messages.as_slice() else {
        panic!("{:?}", turn.messages);
    };
    assert_eq!(said.content.as_str(), "Never mind the extra step.");
    assert_eq!(reply.content.as_str(), "Drafted, then dropped.");
}

/// I5: a turn about a journey that does not exist is refused, naming it.
#[tokio::test]
async fn a_turn_about_a_missing_journey_is_refused() {
    let world = World::new().await;
    let missing = world
        .assistant
        .turn(&user("u_lead"), vendor_target(), "Hello.".parse().unwrap())
        .await;
    assert_eq!(missing, Err(TurnError::TargetMissing(vendor_target())));
    assert_eq!(world.provider.request_count(), 0);
}

/// Conversations are one per target per user: a user's next turn is sent what they said and
/// were told before, in order, and another user's is not.
#[tokio::test]
async fn a_conversation_carries_on_for_its_user_alone() {
    let world = World::new().await;
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    world.script([
        Step::call("update_journey_state", complete_kickoff("p_kickoff", 1)),
        Step::say("First answer."),
        Step::say("Second answer."),
        Step::say("Other user's answer."),
    ]);
    let first = world.turn(&lead, &vendor_target(), "First question.").await;
    let second = world
        .turn(&lead, &vendor_target(), "Second question.")
        .await;
    let other = world
        .turn(&user("u_other"), &vendor_target(), "Mine.")
        .await;
    assert_eq!(first.conversation, second.conversation);
    assert_ne!(first.conversation, other.conversation);
    let sent = world.provider.requests();
    let history: Vec<(String, String)> = roles(&sent[2])[2..].to_vec();
    assert_eq!(history.len(), 3, "{history:?}");
    assert_eq!(
        history[0],
        ("user".to_owned(), "First question.".to_owned())
    );
    assert_eq!(history[1].0, "assistant");
    // The write's line is replayed with the reply it came with.
    assert!(history[1].1.ends_with("First answer.") && history[1].1.len() > "First answer.".len());
    assert_eq!(
        history[2],
        ("user".to_owned(), "Second question.".to_owned())
    );
    assert_eq!(
        roles(&sent[3])[2..],
        [("user".to_owned(), "Mine.".to_owned())]
    );
}

/// The dialogue replayed is the newest within its byte budget.
#[tokio::test]
async fn only_the_newest_dialogue_within_the_budget_is_replayed() {
    let world = World::new().await;
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    let big = "x".repeat(BODY_BYTES_MAX as usize / 2);
    let turns = DIALOGUE_REPLAY_BYTES_MAX as usize / big.len() + 2;
    world.script((0..=turns).map(|_| Step::say("Ok.")));
    for turn in 0..turns {
        world
            .turn(&lead, &vendor_target(), &format!("{}{big}", turn % 10))
            .await;
    }
    world.turn(&lead, &vendor_target(), "Last.").await;
    let sent = roles(&world.provider.requests()[turns]);
    let texts: Vec<&str> = sent[1..].iter().map(|(_, text)| text.as_str()).collect();
    assert!(
        !texts.iter().any(|text| text.starts_with('0')),
        "the oldest left out"
    );
    assert!(
        texts
            .iter()
            .any(|text| text.starts_with(&((turns - 1) % 10).to_string())),
        "the newest kept"
    );
    assert_eq!(texts.last(), Some(&"Last."));
}

/// PRACTICES, Explicit limits: a reply asking for more tool calls than it may run is answered
/// with a result for every call, the ones past the limit refused.
#[tokio::test]
async fn calls_past_the_per_reply_limit_are_refused() {
    let world = World::new().await;
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    let read = json!({ "journey": VENDOR });
    let count = TOOL_CALL_COUNT_PER_REPLY_MAX as usize + 3;
    world.script([
        Step::calls((0..count).map(|_| ("get_snapshot", read.clone()))),
        Step::say("Looked."),
    ]);
    world
        .turn(&lead, &vendor_target(), "Look at everything.")
        .await;
    let answered = results(&world.provider.requests()[1]);
    assert_eq!(answered.len(), count, "every call has its result");
    let refused = answered
        .iter()
        .filter(|result| result["error"] == "refused")
        .count();
    assert_eq!(refused, 3);
}

/// A reply that did not finish runs none of its tool calls, and the turn says it stopped.
#[tokio::test]
async fn an_unfinished_reply_runs_no_tool_call() {
    for finish in ["length", "content_filter"] {
        let world = World::new().await;
        let lead = user("u_lead");
        world.vendor_journey(&lead).await;
        world.script([
            Step::call("update_journey_state", complete_kickoff("p_cut", 1)).finishing(finish),
        ]);
        let turn = world
            .turn(&lead, &vendor_target(), "Kickoff happened.")
            .await;
        stopped(&turn);
        assert_eq!(turn.writes, [], "{finish}");
        assert_eq!(world.journey_revision(VENDOR).await, 1, "{finish}");
    }
}

/// PRACTICES, Explicit limits: a model still calling tools after the iteration limit ends
/// the turn there.
#[tokio::test]
async fn a_turn_ends_at_its_iteration_limit() {
    let world = World::new().await;
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    let read = || Step::call("get_snapshot", json!({ "journey": VENDOR }));
    world.script((0..=TOOL_LOOP_ITERATION_COUNT_MAX).map(|_| read()));
    let turn = world.turn(&lead, &vendor_target(), "Look around.").await;
    stopped(&turn);
    assert_eq!(
        world.provider.request_count(),
        TOOL_LOOP_ITERATION_COUNT_MAX as usize
    );
}

/// A tool call repeated after failing ends the turn at the third failure; different calls
/// that fail are not counted together.
#[tokio::test]
async fn a_tool_call_repeated_after_failing_ends_the_turn() {
    let world = World::new().await;
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    let bad = || {
        Step::call(
            "get_node",
            json!({ "journey": VENDOR, "node": "n_nowhere" }),
        )
    };
    world.script((0..=TOOL_CALL_FAILURE_REPEAT_COUNT_MAX).map(|_| bad()));
    let turn = world.turn(&lead, &vendor_target(), "Read it.").await;
    stopped(&turn);
    assert_eq!(
        world.provider.request_count(),
        TOOL_CALL_FAILURE_REPEAT_COUNT_MAX as usize
    );

    let world = World::new().await;
    world.vendor_journey(&lead).await;
    let missing = |node: &str| Step::call("get_node", json!({ "journey": VENDOR, "node": node }));
    world.script([
        missing("n_a"),
        missing("n_b"),
        missing("n_c"),
        missing("n_d"),
        Step::say("Gave up."),
    ]);
    let turn = world.turn(&lead, &vendor_target(), "Read them.").await;
    assert_eq!(turn.ended, Ended::Replied);
}

/// The results a turn resends are bounded: the newest are kept whole and the oldest replaced
/// by a note.
#[tokio::test]
async fn a_turns_tool_results_are_bounded_between_provider_calls() {
    let world = World::new().await;
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    let read = json!({ "route": "vendor-evaluation", "version": 1 });
    let size = world
        .ok(&lead, "get_route", read.clone())
        .await
        .to_string()
        .len();
    let reads = TOOL_RESULT_BYTES_PER_TURN_MAX as usize / size + 2;
    let per_reply = 16;
    let replies = reads.div_ceil(per_reply);
    assert!(
        replies < TOOL_LOOP_ITERATION_COUNT_MAX as usize,
        "{replies} replies of {size} bytes"
    );
    let rounds = replies * per_reply;
    world.script(
        (0..replies)
            .map(|_| Step::calls((0..per_reply).map(|_| ("get_route", read.clone()))))
            .chain([Step::say("Done.")]),
    );
    world.turn(&lead, &vendor_target(), "Read a lot.").await;
    let last = world.provider.requests().pop().unwrap();
    let kept: usize = last["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["role"] == "tool")
        .map(|message| message["content"].as_str().unwrap().len())
        .sum();
    assert!(kept <= TOOL_RESULT_BYTES_PER_TURN_MAX as usize + DROPPED.len() * rounds);
    let dropped = last["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["content"] == DROPPED)
        .count();
    assert!(dropped > 0, "the oldest results were dropped");
}

/// A provider that refuses the key, fails, or says too much ends the turn as stopped, and
/// what it said never reaches the answer or the conversation.
#[tokio::test]
async fn a_provider_failure_stops_the_turn_without_echoing_what_it_said() {
    for status in [401, 429, 500] {
        let world = World::new().await;
        let lead = user("u_lead");
        world.vendor_journey(&lead).await;
        world.script([Step::status(status)]);
        let turn = world.turn(&lead, &vendor_target(), "Hello.").await;
        let why = stopped(&turn);
        assert!(!why.contains(KEY), "{status}: {why}");
        assert_eq!(why.contains("not set up"), status == 401, "{status}: {why}");
        let held = world
            .store
            .conversation(&turn.conversation)
            .await
            .unwrap()
            .unwrap();
        assert!(
            held.messages
                .iter()
                .all(|message| !message.content.as_str().contains(KEY))
        );
        assert_eq!(world.provider.request_count(), 1, "{status}: no retry");
    }
}

/// PRACTICES, Explicit limits: a provider call past its limit ends the turn, and the writes
/// before it stand.
#[tokio::test(start_paused = true)]
async fn a_provider_timeout_is_reported_and_the_writes_before_it_stand() {
    let world = World::new().await;
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    world.script([
        Step::call("update_journey_state", complete_kickoff("p_kickoff", 1)),
        Step::hang(),
    ]);
    let turn = world
        .turn(&lead, &vendor_target(), "Kickoff happened.")
        .await;
    assert!(stopped(&turn).contains("did not answer"));
    assert_eq!(turn.writes.len(), 1);
    assert_eq!(world.journey_revision(VENDOR).await, 2);
}

/// PRACTICES, Explicit limits: a turn that runs past its duration ends at its next provider
/// call, and no tool call runs past its deadline.
#[tokio::test(start_paused = true)]
async fn a_turn_ends_at_its_duration_limit() {
    let world = World::new().await;
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    let read = || Step::call("get_snapshot", json!({ "journey": VENDOR }));
    world.script((0..10).map(|_| read().after(Duration::from_secs(100))));
    let turn = world.turn(&lead, &vendor_target(), "Look slowly.").await;
    assert!(stopped(&turn).contains("time limit"));
    assert_eq!(
        world.provider.request_count(),
        6,
        "six calls fill ten minutes"
    );
}

/// PRACTICES, Explicit limits: past the turns in flight, a turn is refused at once.
#[tokio::test(start_paused = true)]
async fn turns_past_the_in_flight_limit_are_refused() {
    let world = Arc::new(World::new().await);
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    world.script((0..TURN_IN_FLIGHT_COUNT_MAX).map(|_| Step::hang()));
    let mut running = Vec::new();
    for index in 0..TURN_IN_FLIGHT_COUNT_MAX {
        let world = Arc::clone(&world);
        // One user each: a conversation runs one turn at a time.
        let member = user(&format!("u_member_{index}"));
        running.push(tokio::spawn(async move {
            world.turn(&member, &vendor_target(), "Wait.").await
        }));
    }
    while world.provider.request_count() < TURN_IN_FLIGHT_COUNT_MAX as usize {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    let refused = world
        .assistant
        .turn(&lead, vendor_target(), "One more.".parse().unwrap())
        .await;
    assert_eq!(refused, Err(TurnError::Overloaded));
    for turn in running {
        stopped(&turn.await.unwrap());
    }
}

/// A conversation runs one turn at a time, so a second submission cannot lose the first's
/// messages; another user's conversation is not held up.
#[tokio::test(start_paused = true)]
async fn a_conversation_runs_one_turn_at_a_time() {
    let world = Arc::new(World::new().await);
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    world.script([Step::hang(), Step::say("Hello.")]);
    let first = {
        let (world, lead) = (Arc::clone(&world), lead.clone());
        tokio::spawn(async move { world.turn(&lead, &vendor_target(), "First.").await })
    };
    while world.provider.request_count() == 0 {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    let again = world
        .assistant
        .turn(&lead, vendor_target(), "Again.".parse().unwrap())
        .await;
    assert_eq!(again, Err(TurnError::ConversationBusy));
    let other = world.turn(&user("u_other"), &vendor_target(), "Hi.").await;
    assert_eq!(other.ended, Ended::Replied);
    stopped(&first.await.unwrap());
}

/// I5 (every direct write is reported): a commit the turn's deadline overtakes, but which
/// lands within the save reserve, is reported with the turn that ended at its limit.
#[tokio::test(start_paused = true)]
async fn a_commit_landing_just_past_the_deadline_is_reported() {
    let faults = cairn_store::Faults::default();
    let world = World::with_faults(faults.clone()).await;
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    // A second past the turn's deadline (its limit less the save reserve).
    let lands = tokio::time::Instant::now() + TURN_DURATION_MAX - TURN_SAVE_RESERVE
        + Duration::from_secs(1);
    faults.pause_with(Arc::new(move |_| Box::pin(tokio::time::sleep_until(lands))));
    world.script([
        Step::call("update_journey_state", complete_kickoff("p_late", 1)),
        Step::say("Hello."),
    ]);
    let turn = world
        .turn(&lead, &vendor_target(), "Kickoff happened.")
        .await;
    assert!(stopped(&turn).contains("time limit"));
    assert_eq!(turn.writes.len(), 1, "the late write is reported");
}

/// PRACTICES, Explicit limits: a tool call whose commit stalls is held to the turn's limit;
/// the turn ends, releases its conversation and its slot, and is saved.
#[tokio::test(start_paused = true)]
async fn a_stalled_commit_cannot_hold_a_turn_past_its_limit() {
    let faults = cairn_store::Faults::default();
    let world = World::with_faults(faults.clone()).await;
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    faults.pause_with(Arc::new(|_| Box::pin(std::future::pending())));
    world.script([
        Step::call("update_journey_state", complete_kickoff("p_stalled", 1)),
        Step::say("Hello."),
    ]);
    let started = tokio::time::Instant::now();
    let turn = world
        .turn(&lead, &vendor_target(), "Kickoff happened.")
        .await;
    assert!(stopped(&turn).contains("time limit"));
    assert!(started.elapsed() <= TURN_DURATION_MAX);
    let held = world.store.conversation(&turn.conversation).await.unwrap();
    assert!(held.is_some(), "the conversation is saved");
    let again = world.turn(&lead, &vendor_target(), "Hello?").await;
    assert_eq!(
        again.ended,
        Ended::Replied,
        "the conversation is free again"
    );
}
