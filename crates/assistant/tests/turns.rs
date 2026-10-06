//! Turns (I5, I7, H2, A12, B10; PRD Success criteria): a scripted model drives the tool loop
//! through the write wrapper over the memory store, and each turn reports its writes, keeps
//! its conversation, and ends within its limits.
#![cfg(test)]

mod support;

use std::time::Duration;

use cairn_assistant::limits::{TOOL_LOOP_ITERATION_COUNT_MAX, TURN_IN_FLIGHT_COUNT_MAX};
use cairn_assistant::scripted::Step;
use cairn_assistant::{Action, Because, Ended, Message, Reply, Target, TurnError};
use cairn_store::{ConversationStore, MessageAuthor};
use serde_json::{Value, json};

use support::{World, proposed_mutations, scenario, step_mutations, user, vendor_target};

const BAKEOFF: &str = "j_bakeoff";

fn bakeoff() -> Target {
    Target::Journey(BAKEOFF.parse().unwrap())
}

/// The events of the journey's last patch, from its history.
async fn last_events(world: &World, journey: &str) -> Vec<Value> {
    let history = world
        .ok(
            &user("u_reader"),
            "get_history",
            json!({ "journey": journey }),
        )
        .await;
    let patches = history["patches"].as_array().unwrap();
    patches.last().unwrap()["events"]
        .as_array()
        .unwrap()
        .clone()
}

/// The PRD's success criterion with no route (I5, I7, H2): the owner starts an empty
/// journey and describes the work; the assistant reads it and writes the structure, which
/// arrives as one proposal while the journey stays where it was; the owner applies it, and
/// the events record the assistant as the proposing agent and the owner as confirming.
#[tokio::test]
async fn an_empty_journey_is_structured_through_a_proposal_the_user_applies() {
    let world = World::new();
    world.empty_bakeoff().await;
    let owner = user("u_owner");
    let structure = proposed_mutations(&scenario("bake-off"), 1);
    let patch = json!({ "patch": { "id": "p_structure", "target": { "journey": BAKEOFF },
        "base_revision": 1, "mutations": structure }, "note": "Structure the bake-off" });
    world.script([
        Step::call("get_snapshot", json!({ "journey": BAKEOFF })),
        Step::call("apply_patch", patch),
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
    let [
        Action::Proposed {
            proposal, because, ..
        },
    ] = turn.actions.as_slice()
    else {
        panic!("{:?}", turn.actions);
    };
    assert_eq!(*because, Because::Structural);
    assert_eq!(
        world.journey_revision(BAKEOFF).await,
        1,
        "drafting moves nothing"
    );

    let apply = json!({ "proposal": proposal, "reviewed_revision": 1, "patch_id": "p_apply" });
    world.ok(&owner, "apply_proposal", apply).await;
    assert_eq!(world.journey_revision(BAKEOFF).await, 2);
    let snapshot = world
        .ok(&owner, "get_snapshot", json!({ "journey": BAKEOFF }))
        .await;
    let open = snapshot["snapshot"]["open_decisions"].as_array().unwrap();
    assert!(
        !open.is_empty(),
        "the structure's decisions are open: {snapshot}"
    );
    let events = last_events(&world, BAKEOFF).await;
    assert!(
        events
            .iter()
            .all(|event| event["confirming_user"] == "u_owner")
    );
    let held = world
        .ok(&owner, "get_proposal", json!({ "proposal": proposal }))
        .await;
    assert_eq!(held["proposal"]["proposing_agent"], "ag_assistant");
    assert_eq!(held["proposal"]["created_by"], "u_owner");
}

/// I5, D7, H2: a state change the user asks for applies directly and is reported with what
/// it caused; its events record the assistant and the user it acts for.
#[tokio::test]
async fn a_direct_state_change_is_reported_with_its_consequences() {
    let world = World::new();
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    let answers = step_mutations(&scenario("vendor-evaluation"), 1);
    let mut answers: Vec<Value> = answers;
    // A meeting far sooner than the work feeding it can be done.
    for answer in &mut answers {
        if answer["decision"] == "n_meeting_date" {
            answer["value"] = json!({ "date": "2026-10-06" });
        }
    }
    let patch = json!({ "patch": { "id": "p_answers", "target": { "journey": "j_vendor_eval" },
        "base_revision": 1, "deployment_revision": 1, "mutations": answers } });
    world.script([
        Step::call("apply_patch", patch),
        Step::say("Answered; the decision meeting is now too soon for the work before it."),
    ]);
    let turn = world
        .turn(
            &lead,
            &vendor_target(),
            "Answer the up-front decisions; meeting on the 6th.",
        )
        .await;
    let [
        Action::Applied {
            receipt,
            consequences,
            ..
        },
    ] = turn.actions.as_slice()
    else {
        panic!("{:?}", turn.actions);
    };
    assert_eq!(receipt.revision.get(), 2);
    let caused = consequences.get(&"j_vendor_eval".parse().unwrap());
    assert!(
        caused.is_some_and(|caused| !caused.shortfalls.is_empty()),
        "{consequences:?}"
    );
    let events = last_events(&world, "j_vendor_eval").await;
    for event in events {
        assert_eq!(
            event["actor"],
            json!({ "user": "u_lead", "agent": "ag_assistant" })
        );
    }
}

/// I5: a bulk request past ten nodes, asked in conversation, is one proposal.
#[tokio::test]
async fn a_bulk_request_in_conversation_becomes_one_proposal() {
    let world = World::new();
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    let nodes = [
        "n_access",
        "n_plan",
        "n_plan_draft",
        "n_plan_review",
        "n_workload",
        "n_baseline",
        "n_criteria",
        "n_partner_results",
        "n_findings",
        "n_final_report",
        "n_decision_meeting",
    ];
    let mutations: Vec<Value> = nodes
        .iter()
        .map(|node| json!({ "op": "set_node_field", "node": node, "value": { "weight": 2 } }))
        .collect();
    let patch = json!({ "patch": { "id": "p_reweigh", "target": { "journey": "j_vendor_eval" },
        "base_revision": 1, "mutations": mutations } });
    world.script([
        Step::call("apply_patch", patch),
        Step::say("Drafted for review."),
    ]);
    let turn = world
        .turn(
            &lead,
            &vendor_target(),
            "Lower every work item's weight to 2.",
        )
        .await;
    let [Action::Proposed { because, .. }] = turn.actions.as_slice() else {
        panic!("{:?}", turn.actions);
    };
    assert_eq!(*because, Because::TooManyNodes { count: Some(11) });
    assert_eq!(world.journey_revision("j_vendor_eval").await, 1);
}

/// A12: a route drafted in conversation arrives as a proposal on its draft, creating the
/// route when it does not exist yet; applied, the draft holds what was drafted.
#[tokio::test]
async fn a_route_drafted_in_conversation_arrives_as_a_proposal_on_its_draft() {
    let world = World::new();
    let author = user("u_author");
    let node = |key: &str, id: &str, title: &str| {
        json!({ "op": "add_node", "node": { "key": key, "id": id, "kind": "action",
            "title": title } })
    };
    let mutations = json!([
        { "op": "create_route", "name": "Checklist" },
        { "op": "open_draft", "source": "edit" },
        node("n_gather", "gather", "Gather what is needed"),
        node("n_check", "check", "Check it"),
    ]);
    let patch = json!({ "patch": { "id": "p_checklist", "target": { "route": "checklist" },
        "base_revision": 0, "mutations": mutations } });
    world.script([
        Step::call("apply_patch", patch),
        Step::say("Drafted the checklist."),
    ]);
    let target = Target::RouteDraft("checklist".parse().unwrap());
    let turn = world
        .turn(&author, &target, "A two-step checklist route.")
        .await;
    let [
        Action::Proposed {
            proposal, because, ..
        },
    ] = turn.actions.as_slice()
    else {
        panic!("{:?}", turn.actions);
    };
    assert_eq!(*because, Because::Structural);
    assert_eq!(
        world.route_revision("checklist").await,
        0,
        "nothing exists until applied"
    );
    let context = &world.provider.exchanges()[0].system;
    assert!(
        context.contains("not_found"),
        "the turn read the missing draft"
    );

    let apply = json!({ "proposal": proposal, "reviewed_revision": 1, "patch_id": "p_apply" });
    world.ok(&author, "apply_proposal", apply).await;
    let draft = world
        .ok(&author, "get_route", json!({ "route": "checklist" }))
        .await;
    assert_eq!(draft["graph"]["nodes"]["total"], 2, "{draft}");
}

/// B10: the assisted per-member breakdown drafts one child per member of a role, each with
/// an explicit participation for its member, which the applied proposal keeps.
#[tokio::test]
async fn a_per_member_breakdown_writes_explicit_participations() {
    let world = World::new();
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    let members = ["e_stakeholder_a", "e_stakeholder_b"];
    let mut mutations = Vec::new();
    for (index, member) in members.iter().enumerate() {
        let key = format!("n_workload_member_{index}");
        mutations.push(json!({ "op": "add_node", "node": { "key": key,
            "id": format!("member-{index}"), "parent": "n_workload", "kind": "deliverable",
            "title": format!("Workload for member {index}") } }));
        mutations.push(
            json!({ "op": "set_participation", "node": key, "kind": "k_owner",
            "source": [member] }),
        );
    }
    let draft = json!({ "title": "One workload per stakeholder", "destination_revision": 1,
        "mutations": mutations });
    let create = json!({ "proposal": "pr_per_member", "destination": { "journey": "j_vendor_eval" },
        "draft": draft, "patch_id": "p_per_member" });
    world.script([
        Step::call("create_proposal", create),
        Step::say("Proposed."),
    ]);
    let turn = world
        .turn(
            &lead,
            &vendor_target(),
            "Break the workload down per stakeholder.",
        )
        .await;
    assert!(matches!(
        turn.actions.as_slice(),
        [Action::Proposed {
            because: Because::Asked,
            ..
        }]
    ));
    let apply = json!({ "proposal": "pr_per_member", "reviewed_revision": 1,
        "patch_id": "p_apply_per_member" });
    world.ok(&lead, "apply_proposal", apply).await;
    for (index, member) in members.iter().enumerate() {
        let node = format!("n_workload_member_{index}");
        let detail = world
            .ok(
                &lead,
                "get_node",
                json!({ "journey": "j_vendor_eval", "node": node }),
            )
            .await;
        let text = detail.to_string();
        assert!(text.contains(member), "{node} is {member}'s: {text}");
    }
}

/// A provider that does not answer is reported as timed out once the provider call limit
/// passes; the writes made before it stand and are reported, and the conversation keeps a
/// note of how the turn ended.
#[tokio::test(start_paused = true)]
async fn a_provider_timeout_is_reported_and_the_writes_before_it_stand() {
    let world = World::new();
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    let reach = json!({ "journey": "j_vendor_eval", "node": "n_kickoff", "transition": "reach",
        "patch_id": "p_kickoff", "base_revision": 1 });
    world.script([Step::call("transition_node", reach), Step::Hang]);
    let turn = world
        .turn(&lead, &vendor_target(), "Kickoff happened.")
        .await;
    assert_eq!(turn.ended, Ended::ProviderTimedOut);
    assert!(matches!(turn.actions.as_slice(), [Action::Applied { .. }]));
    assert_eq!(world.journey_revision("j_vendor_eval").await, 2);
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
        "the user's words, the write, and how the turn ended"
    );
}

/// PRACTICES, Explicit limits: a model still calling tools after the iteration limit ends
/// the turn there, and a turn that runs past its duration ends at its next provider call.
#[tokio::test(start_paused = true)]
async fn a_turn_ends_at_its_iteration_and_duration_limits() {
    let world = World::new();
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    let read = || Step::call("get_snapshot", json!({ "journey": "j_vendor_eval" }));
    world.script((0..=TOOL_LOOP_ITERATION_COUNT_MAX).map(|_| read()));
    let turn = world.turn(&lead, &vendor_target(), "Look around.").await;
    assert_eq!(turn.ended, Ended::IterationLimit);
    assert_eq!(
        world.provider.exchanges().len(),
        TOOL_LOOP_ITERATION_COUNT_MAX as usize
    );
    assert_eq!(world.provider.remaining(), 1);

    let slow = || {
        let Step::Reply(reply) = read() else {
            unreachable!()
        };
        Step::Slow(Duration::from_secs(100), reply)
    };
    let world = World::new();
    world.vendor_journey(&lead).await;
    world.script((0..10).map(|_| slow()));
    let turn = world.turn(&lead, &vendor_target(), "Look slowly.").await;
    assert_eq!(turn.ended, Ended::TurnTimedOut);
    assert_eq!(
        world.provider.exchanges().len(),
        6,
        "six calls fill ten minutes"
    );
}

/// PRACTICES, Explicit limits: past the turns in flight, a turn is refused at once.
#[tokio::test(start_paused = true)]
async fn turns_past_the_in_flight_limit_are_refused() {
    let world = std::sync::Arc::new(World::new());
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    world.script((0..TURN_IN_FLIGHT_COUNT_MAX).map(|_| Step::Hang));
    let mut running = Vec::new();
    for _ in 0..TURN_IN_FLIGHT_COUNT_MAX {
        let world = std::sync::Arc::clone(&world);
        let lead = lead.clone();
        running.push(tokio::spawn(async move {
            world.turn(&lead, &vendor_target(), "Wait.").await
        }));
    }
    while world.provider.exchanges().len() < TURN_IN_FLIGHT_COUNT_MAX as usize {
        tokio::task::yield_now().await;
    }
    let refused = world
        .assistant
        .turn(&lead, vendor_target(), "One more.".parse().unwrap())
        .await;
    assert_eq!(refused, Err(TurnError::Overloaded));
    for turn in running {
        assert_eq!(turn.await.unwrap().ended, Ended::ProviderTimedOut);
    }
}

/// I7: the assistant acts for a signed-in user; an agent calls the tools itself.
#[tokio::test]
async fn an_agent_caller_is_refused() {
    let world = World::new();
    let helper = support::assisting("u_lead");
    let refused = world
        .assistant
        .turn(&helper, vendor_target(), "Hello.".parse().unwrap())
        .await;
    assert_eq!(refused, Err(TurnError::AgentCaller));
    let missing = world
        .assistant
        .turn(&user("u_lead"), vendor_target(), "Hello.".parse().unwrap())
        .await;
    assert_eq!(missing, Err(TurnError::TargetMissing(vendor_target())));
}

/// Conversations are one per target per user: a user's next turn is sent what they said
/// before, and another user's is not.
#[tokio::test]
async fn a_conversation_carries_on_for_its_user_alone() {
    let world = World::new();
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    world.script([
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
    let sent =
        |index: usize| -> Vec<Message> { world.provider.exchanges()[index].messages.clone() };
    let said = |text: &str| Message::User {
        text: text.to_owned(),
    };
    let replied = |text: &str| {
        Message::Assistant(Reply {
            text: Some(text.to_owned()),
            ..Reply::default()
        })
    };
    assert_eq!(
        sent(1),
        [
            said("First question."),
            replied("First answer."),
            said("Second question.")
        ]
    );
    assert_eq!(sent(2), [said("Mine.")]);
}
