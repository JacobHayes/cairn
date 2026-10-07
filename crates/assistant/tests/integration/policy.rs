//! The write wrapper's policy (I5, I7; PRD glossary, Structural change): every write tool
//! the tool set lists is decided by mutation kind and destination, structural changes and
//! bulk state changes never land directly, and applying a proposal is left to the user.
//! The tables iterate every write tool, so a write tool added without a case fails here.
#![cfg(test)]

use crate::support;

use std::collections::BTreeSet;

use cairn_assistant::wrapper::{self, Action, Because, Decision, Ledger, Outcome, Policy};
use cairn_mcp::{ToolError, ToolSet};
use cairn_store::MemoryStore;
use serde_json::{Value, json};

use support::{World, assisting, user};

const JOURNEY: &str = "j_vendor_eval";
const ROUTE: &str = "vendor-evaluation";

/// The weight-override patch: one `set_node_field` weight per node, to the journey.
fn weights(nodes: &[&str], patch_id: &str, base: u64) -> Value {
    let mutations: Vec<Value> = nodes
        .iter()
        .map(|node| json!({ "op": "set_node_field", "node": node, "value": { "weight": 3 } }))
        .collect();
    json!({ "patch": { "id": patch_id, "target": { "journey": JOURNEY }, "base_revision": base,
        "mutations": mutations } })
}

const ELEVEN: [&str; 11] = [
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

/// A case: a write tool, its arguments, and the policy it must get.
type Case = (&'static str, Value, Policy);

/// One case per write tool: the call, as a model would make it on the vendor evaluation at
/// revision 1 with its draft open at `draft`, and the policy it must get. A structural
/// variant is chosen wherever the tool can carry one.
fn cases(draft: u64) -> Vec<Case> {
    let mut cases = state_cases();
    cases.extend(structural_cases(draft));
    cases.extend(proposal_cases());
    cases
}

/// The journey's arguments with `extra` added.
fn on_journey(extra: Value) -> Value {
    let mut arguments = json!({ "journey": JOURNEY, "base_revision": 1 });
    if let Value::Object(extra) = extra {
        arguments.as_object_mut().unwrap().extend(extra);
    }
    arguments
}

/// A structural mutation: a node added.
fn add_node() -> Value {
    json!({ "op": "add_node", "node": { "key": "n_extra", "id": "extra",
        "kind": "action", "title": "An extra step" } })
}

/// The journey's state tools: one node each, so each applies directly.
fn state_cases() -> Vec<Case> {
    let journey = on_journey;
    vec![
        (
            "answer_decision",
            journey(
                json!({ "decision": "n_purpose", "value": { "single_choice": "purchase" },
                "patch_id": "p_answer" }),
            ),
            Policy::Direct,
        ),
        (
            "transition_node",
            journey(json!({ "node": "n_kickoff", "transition": "reach", "patch_id": "p_reach" })),
            Policy::Direct,
        ),
        (
            "assign",
            journey(
                json!({ "node": "n_access", "kind": "k_reviewer", "source": ["e_lead"],
                "deployment_revision": 1, "patch_id": "p_assign" }),
            ),
            Policy::Direct,
        ),
        (
            "snooze",
            journey(
                json!({ "node": "n_access", "until": { "date": "2026-10-03" },
                "patch_id": "p_snooze" }),
            ),
            Policy::Direct,
        ),
        (
            "unsnooze",
            journey(json!({ "node": "n_access", "patch_id": "p_unsnooze" })),
            Policy::Direct,
        ),
        (
            "set_date",
            journey(
                json!({ "node": "n_findings", "change": { "pin": { "date": "2026-11-03" } },
                "patch_id": "p_pin" }),
            ),
            Policy::Direct,
        ),
        (
            "override",
            journey(
                json!({ "node": "n_partner_led", "change": { "apply": { "force_include":
                { "reason": "The partner may run part of it." } } }, "patch_id": "p_force" }),
            ),
            Policy::Direct,
        ),
    ]
}

/// The writes that carry structure, each drafted as a proposal.
fn structural_cases(draft: u64) -> Vec<Case> {
    let journey = on_journey;
    let structural = Policy::Propose(Because::Structural);
    let add_node = add_node();
    vec![
        (
            "resolve_date_conflict",
            journey(
                json!({ "resolution": { "op": "remove_edge", "edge": { "node": "n_findings",
                "requires": "n_baseline" } }, "patch_id": "p_resolve" }),
            ),
            structural.clone(),
        ),
        (
            "create_journey",
            json!({ "journey": "j_new", "name": "A new journey", "patch_id": "p_new_journey" }),
            structural.clone(),
        ),
        (
            "apply_patch",
            json!({ "patch": { "id": "p_add", "target": { "journey": JOURNEY },
                "base_revision": 1, "mutations": [add_node] } }),
            structural.clone(),
        ),
        (
            "open_draft",
            json!({ "route": "fresh", "create": { "name": "Fresh" }, "patch_id": "p_fresh",
                "base_revision": 0 }),
            structural.clone(),
        ),
        (
            "publish_draft",
            json!({ "route": ROUTE, "patch_id": "p_publish", "base_revision": draft }),
            structural.clone(),
        ),
        (
            "manage_entity",
            json!({ "change": { "merge": { "survivor": "e_lead", "merged": "e_analyst" } },
                "patch_id": "p_merge", "base_revision": 1 }),
            structural.clone(),
        ),
    ]
}

/// The refused tools and the proposal tools.
fn proposal_cases() -> Vec<Case> {
    let add_node = add_node();
    vec![
        (
            "import_route",
            json!({ "patch_id": "p_import", "file": "format: 1" }),
            Policy::Refused(String::new()),
        ),
        (
            "apply_proposal",
            json!({ "proposal": "pr_any", "reviewed_revision": 1, "patch_id": "p_apply" }),
            Policy::Refused(String::new()),
        ),
        (
            "create_proposal",
            json!({ "proposal": "pr_mine", "destination": { "journey": JOURNEY },
                "draft": { "title": "Add a step", "destination_revision": 1,
                    "mutations": [add_node] }, "patch_id": "p_mine" }),
            Policy::Proposal,
        ),
        (
            "edit_proposal",
            json!({ "proposal": "pr_mine", "change": "discard", "base_revision": 1,
                "patch_id": "p_discard_mine" }),
            Policy::Proposal,
        ),
        (
            "upgrade",
            json!({ "journey": JOURNEY, "to": 1, "proposal": "pr_upgrade",
                "patch_id": "p_upgrade" }),
            Policy::Proposal,
        ),
        (
            "save_as_route",
            json!({ "journey": JOURNEY, "route": "saved", "name": "Saved",
                "proposal": "pr_save", "patch_id": "p_save" }),
            Policy::Proposal,
        ),
        (
            "relink",
            json!({ "journey": JOURNEY, "to": { "route": ROUTE, "version": 1 },
                "proposal": "pr_relink", "patch_id": "p_relink" }),
            Policy::Proposal,
        ),
    ]
}

/// Runs one call through the wrapper as a turn of its own.
async fn run(world: &World, actor: &cairn_schema::Actor, name: &str, arguments: Value) -> Outcome {
    let store = world.store.as_ref();
    let mut ledger = Ledger::default();
    wrapper::run(&world.tools, store, actor, (name, arguments), &mut ledger).await
}

/// Whether two policies are the same, a refusal's reason aside.
fn same(policy: &Policy, expected: &Policy) -> bool {
    match (policy, expected) {
        (Policy::Refused(_), Policy::Refused(_)) => true,
        _ => policy == expected,
    }
}

/// I5, I7: every write tool gets the policy its kind and destination call for, a structural
/// variant wherever it has one; the cases cover exactly the tool set's write tools.
#[tokio::test]
async fn every_write_tool_is_decided_by_mutation_kind_and_destination() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let draft = world.open_vendor_draft(&ann).await;
    let cases = cases(draft);
    let covered: BTreeSet<&str> = cases.iter().map(|(name, _, _)| *name).collect();
    let writes: BTreeSet<&str> = ToolSet::<MemoryStore>::definitions()
        .into_iter()
        .filter(|definition| definition.writes)
        .map(|definition| definition.name)
        .collect();
    assert_eq!(covered, writes, "a case for every write tool, and no other");
    for (name, arguments, expected) in &cases {
        let decided = wrapper::decide(&world.tools, world.store.as_ref(), name, arguments).await;
        let policy = decided.map(|decision| decision.policy());
        assert!(
            matches!(&policy, Ok(policy) if same(policy, expected)),
            "{name}: {policy:?}, expected {expected:?}"
        );
    }
    for read in ToolSet::<MemoryStore>::definitions()
        .into_iter()
        .filter(|definition| !definition.writes)
    {
        let decided =
            wrapper::decide(&world.tools, world.store.as_ref(), read.name, &json!({})).await;
        assert_eq!(decided, Ok(Decision::Read), "{}", read.name);
    }
}

/// I5: a write drafted as a proposal never moves its destination: what it would have
/// written is the proposal's content, against the same revision, for the user to apply; the
/// refused tools write nothing at all.
#[tokio::test]
async fn structural_writes_arrive_as_proposals_and_nothing_lands() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let draft = world.open_vendor_draft(&ann).await;
    let assistant = assisting("u_ann");
    for (name, arguments, expected) in cases(draft) {
        if matches!(expected, Policy::Direct | Policy::Proposal | Policy::Read) {
            continue;
        }
        let journey = world.journey_revision(JOURNEY).await;
        let route = world.route_revision(ROUTE).await;
        let outcome = run(&world, &assistant, name, arguments).await;
        match expected {
            Policy::Propose(because) => {
                let Some(Action::Proposed {
                    proposal,
                    because: why,
                    ..
                }) = outcome.action
                else {
                    panic!("{name}: {outcome:?}");
                };
                assert_eq!(why, because, "{name}");
                let held = world
                    .ok(&ann, "get_proposal", json!({ "proposal": proposal }))
                    .await;
                assert_eq!(held["proposal"]["status"], "open", "{name}");
                assert_eq!(
                    held["proposal"]["proposing_agent"], "ag_assistant",
                    "{name}"
                );
            }
            Policy::Refused(_) => {
                let refused = matches!(outcome.answer, Err(ToolError::Refused { .. }));
                assert!(refused && outcome.action.is_none(), "{name}: {outcome:?}");
            }
            Policy::Direct | Policy::Proposal | Policy::Read => unreachable!(),
        }
        assert_eq!(
            world.journey_revision(JOURNEY).await,
            journey,
            "{name} moved the journey"
        );
        assert_eq!(
            world.route_revision(ROUTE).await,
            route,
            "{name} moved the route"
        );
    }
}

/// I5's policy by destination: a weight change on a route draft is a proposal; the same
/// change as a journey override applies directly and reports its consequences (B5, D7).
#[tokio::test]
async fn a_weight_change_is_proposed_on_a_draft_and_applied_on_a_journey() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let draft = world.open_vendor_draft(&ann).await;
    let assistant = assisting("u_ann");
    let weight = json!({ "op": "set_node_field", "node": "n_findings", "value": { "weight": 7 } });
    let on_draft = json!({ "patch": { "id": "p_draft_weight", "target": { "route": ROUTE },
        "base_revision": draft, "mutations": [weight] } });
    let proposed = run(&world, &assistant, "apply_patch", on_draft).await;
    assert!(
        matches!(
            proposed.action,
            Some(Action::Proposed {
                because: Because::Structural,
                ..
            })
        ),
        "{proposed:?}"
    );
    assert_eq!(world.route_revision(ROUTE).await, draft);

    let on_journey = json!({ "patch": { "id": "p_journey_weight", "target": { "journey": JOURNEY },
        "base_revision": 1, "mutations": [weight] } });
    let applied = run(&world, &assistant, "apply_patch", on_journey).await;
    let Some(Action::Applied { receipt, .. }) = applied.action else {
        panic!("{applied:?}");
    };
    assert_eq!(receipt.revision.get(), 2);
    assert_eq!(world.journey_revision(JOURNEY).await, 2);
}

/// I5: a state change touching more than ten nodes is one proposal, so a misheard bulk
/// request is seen whole; ten apply at once.
#[tokio::test]
async fn a_state_change_past_ten_nodes_becomes_one_proposal() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let assistant = assisting("u_ann");
    let eleven = run(
        &world,
        &assistant,
        "apply_patch",
        weights(&ELEVEN, "p_eleven", 1),
    )
    .await;
    let Some(Action::Proposed {
        proposal, because, ..
    }) = eleven.action
    else {
        panic!("{eleven:?}");
    };
    assert_eq!(because, Because::TooManyNodes { count: Some(11) });
    assert_eq!(world.journey_revision(JOURNEY).await, 1, "nothing landed");
    let held = world
        .ok(&ann, "get_proposal", json!({ "proposal": proposal }))
        .await;
    let mutations = held["proposal"]["draft"]["mutations"].as_array().unwrap();
    assert_eq!(
        mutations.len(),
        ELEVEN.len(),
        "the whole change, in one proposal"
    );

    let ten = run(
        &world,
        &assistant,
        "apply_patch",
        weights(&ELEVEN[..10], "p_ten", 1),
    )
    .await;
    assert!(
        matches!(ten.action, Some(Action::Applied { .. })),
        "{ten:?}"
    );
    assert_eq!(world.journey_revision(JOURNEY).await, 2);
}

/// H5, I6: the same write asked twice is filed under the same proposal, not a second one.
#[tokio::test]
async fn a_write_asked_again_answers_the_same_proposal() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let assistant = assisting("u_ann");
    let mut proposals = BTreeSet::new();
    for _ in 0..2 {
        let outcome = run(
            &world,
            &assistant,
            "apply_patch",
            weights(&ELEVEN, "p_again", 1),
        )
        .await;
        let Some(Action::Proposed { proposal, .. }) = outcome.action else {
            panic!("{outcome:?}");
        };
        proposals.insert(proposal);
    }
    assert_eq!(proposals.len(), 1);
}

/// I5 (review 4.4 r1): removing notes counts the nodes they are on, read from the stored
/// journey, as adding them does; skipping a container counts the subtree it skips (D1a).
#[tokio::test]
async fn removals_and_skips_count_the_nodes_they_reach() {
    let world = World::new();
    let lead = user("u_lead");
    world.vendor_journey(&lead).await;
    let mut twelve = ELEVEN.to_vec();
    twelve.push("n_kickoff");
    let adds: Vec<Value> = twelve
        .iter()
        .enumerate()
        .map(|(index, node)| {
            json!({ "op": "add_annotation", "annotation": { "key": format!("a_note_{index}"),
                "node": node, "note": "Mine." } })
        })
        .collect();
    let added = json!({ "patch": { "id": "p_notes", "target": { "journey": JOURNEY },
        "base_revision": 1, "mutations": adds } });
    world.ok(&lead, "apply_patch", added).await;
    let removes: Vec<Value> = (0..twelve.len())
        .map(|index| json!({ "op": "remove_annotation", "annotation": format!("a_note_{index}") }))
        .collect();
    let removal = json!({ "patch": { "id": "p_unnote", "target": { "journey": JOURNEY },
        "base_revision": 2, "mutations": removes } });
    let decided =
        wrapper::decide(&world.tools, world.store.as_ref(), "apply_patch", &removal).await;
    assert_eq!(
        decided.map(|decision| decision.policy()),
        Ok(Policy::Propose(Because::TooManyNodes { count: Some(12) }))
    );

    let skip = json!({ "journey": JOURNEY, "node": "n_setup",
        "transition": { "skip": { "reason": "Not needed." } }, "patch_id": "p_skip_setup",
        "base_revision": 2 });
    let decided =
        wrapper::decide(&world.tools, world.store.as_ref(), "transition_node", &skip).await;
    let Ok(Decision::Direct { nodes, .. }) = decided else {
        panic!("{decided:?}");
    };
    assert!(nodes.len() > 1, "the group and what it contains: {nodes:?}");
    assert!(nodes.contains(&"n_access".parse().unwrap()));
}
