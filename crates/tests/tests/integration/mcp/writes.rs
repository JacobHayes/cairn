//! The direct-write tools in process over the memory store: each is one patch, refused with
//! every violation (A15), answered stale with what moved or from its receipt when
//! resubmitted (H5), and recorded as the agent and the user it acts for (I7).
//! `update_journey_state` takes what happened in one journey and refuses anything else by its
//! path; `apply_patch` takes any patch, state and structure in one.
#![cfg(test)]

use crate::mcp::support;

use std::collections::BTreeSet;

use cairn_mcp::ToolError;
use cairn_schema::{GuardFailure, Rejection, Subject, ViolationCode};
use serde_json::{Value, json};

use support::{World, agent, user};

/// `update_journey_state`'s arguments: `mutations` to the vendor evaluation at `base`.
fn state(patch_id: &str, base: u32, mutations: Value) -> Value {
    let mut arguments = json!({ "journey": "j_vendor_eval", "patch_id": patch_id,
        "base_revision": base });
    arguments["mutations"] = mutations;
    arguments
}

/// `apply_patch`'s arguments: `mutations` to the vendor evaluation at `base`.
fn patch(patch_id: &str, base: u32, mutations: Value) -> Value {
    let mut arguments = json!({ "target": { "journey": "j_vendor_eval" }, "patch_id": patch_id,
        "base_revision": base });
    arguments["mutations"] = mutations;
    arguments
}

/// A15: a write with three independent violations is refused with all three, as the engine
/// reported them, and nothing is written.
#[tokio::test]
async fn a_refused_write_carries_every_violation() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let three = json!([
        { "op": "transition", "node": "n_kickoff", "transition": "start" },
        { "op": "transition", "node": "n_nowhere", "transition": "start" },
        { "op": "answer", "decision": "n_partner_runs", "value": { "text": "maybe" } },
    ]);
    let refused = world
        .call(&ann, "update_journey_state", state("p_three", 1, three))
        .await;
    let Err(ToolError::Rejected {
        rejection: Rejection::Invalid { violations },
    }) = refused
    else {
        panic!("{refused:?}");
    };
    let violations = violations.as_slice();
    let codes: BTreeSet<ViolationCode> = violations.iter().map(|v| v.code).collect();
    assert_eq!(violations.len(), 3);
    assert!(codes.contains(&ViolationCode::UnresolvedReference));
    assert!(codes.contains(&ViolationCode::AnswerTypeMismatch));
    let snapshot = world
        .ok(&ann, "get_snapshot", json!({ "journey": "j_vendor_eval" }))
        .await;
    assert_eq!(snapshot["revision"], 1, "nothing was written");
}

/// A direct write takes no mutation that changes a proposal: applying one inside a patch is
/// refused at its path, pointing at the tool that applies it, and nothing is written; nor is
/// a proposal a target.
#[tokio::test]
async fn a_direct_write_applies_no_proposal() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let inside = json!([
        { "op": "transition", "node": "n_kickoff", "transition": "start" },
        { "op": "apply_proposal", "proposal": "pr_any", "reviewed_revision": 1 },
    ]);
    for tool in ["apply_patch", "update_journey_state"] {
        let arguments = if tool == "apply_patch" {
            patch("p_apply_inside", 1, inside.clone())
        } else {
            state("p_apply_inside", 1, inside.clone())
        };
        let refused = world.call(&ann, tool, arguments).await;
        let Err(ToolError::Arguments { path, message }) = refused else {
            panic!("{tool}: {refused:?}");
        };
        assert_eq!(path, "mutations[1].op", "{tool}");
        assert!(message.contains("apply_proposal"), "{tool}: {message}");
    }
    let mut target = patch(
        "p_at_proposal",
        1,
        json!([{ "op": "transition", "node": "n_kickoff", "transition": "start" }]),
    );
    target["target"] =
        json!({ "proposal": { "id": "pr_any", "destination": { "journey": "j_vendor_eval" } } });
    let refused = world.call(&ann, "apply_patch", target).await;
    assert!(
        matches!(&refused, Err(ToolError::Arguments { path, .. }) if path.starts_with("target")),
        "{refused:?}"
    );
    let snapshot = world
        .ok(&ann, "get_snapshot", json!({ "journey": "j_vendor_eval" }))
        .await;
    assert_eq!(snapshot["revision"], 1, "nothing was written");
}

/// B10, A16, D4: completing a placeholder that is not broken down is refused through the
/// tool with the one guard that failed, named on the node, and nothing is
/// written; the same completion going ahead anyway with a reason applies.
#[tokio::test]
async fn completing_a_placeholder_not_broken_down_is_refused_until_bypassed() {
    let world = World::new();
    let ann = user("u_ann");
    // Kickoff done and access started: the workload is open and its stage has opened.
    world.vendor_after(&ann, 4).await;
    let complete = json!([{ "op": "transition", "node": "n_workload", "transition": "complete" }]);
    let complete = state("p_workload_done", 4, complete);
    let refused = world.call(&ann, "update_journey_state", complete).await;
    let Err(ToolError::Rejected {
        rejection: Rejection::Invalid { violations },
    }) = refused
    else {
        panic!("{refused:?}");
    };
    let [violation] = violations.as_slice() else {
        panic!("{violations:#?}");
    };
    let workload = "n_workload".parse().unwrap();
    assert_eq!(violation.code, ViolationCode::GuardFailed);
    assert_eq!(violation.at.subject, Some(Subject::Node(workload)));
    assert_eq!(
        violation.failures,
        BTreeSet::from([GuardFailure::NotBrokenDown])
    );
    let snapshot = json!({ "journey": "j_vendor_eval" });
    let snapshot = world.ok(&ann, "get_snapshot", snapshot).await;
    assert_eq!(snapshot["revision"], 4, "nothing was written");

    let bypassed = json!([{ "op": "transition", "node": "n_workload",
        "transition": { "complete_anyway": { "reason": "Run as one piece." } } }]);
    let bypassed = state("p_workload_bypassed", 4, bypassed);
    let applied = world.ok(&ann, "update_journey_state", bypassed).await;
    assert_eq!(applied["revision"], 5);
}

/// Gives the access node two items and the requirement to tick them, at revision 4.
async fn give_access_a_checklist(world: &World, ann: &cairn_schema::Actor) {
    let given = json!([
        { "op": "set_node_field", "node": "n_access", "value": { "checklist": [
            { "id": "request", "title": "Request sent" },
            { "id": "granted", "title": "Access granted" } ] } },
        { "op": "set_node_field", "node": "n_access", "value": { "requires_checklist": true } },
    ]);
    world
        .ok(ann, "apply_patch", patch("p_items", 4, given))
        .await;
}

/// A16, D4, G5: work with a required checklist is refused through the tool once per unticked
/// item; the ticks and the completion sent as one patch apply, and the node reads back with its
/// ticks and count.
#[tokio::test]
async fn completing_work_with_a_checklist_needs_its_ticks_in_the_same_patch() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_after(&ann, 4).await;
    give_access_a_checklist(&world, &ann).await;
    let complete = json!([{ "op": "transition", "node": "n_access", "transition": "complete" }]);
    let complete = state("p_access_done", 5, complete);
    let refused = world.call(&ann, "update_journey_state", complete).await;
    let Err(ToolError::Rejected {
        rejection: Rejection::Invalid { violations },
    }) = refused
    else {
        panic!("{refused:?}");
    };
    let failures: BTreeSet<GuardFailure> = violations
        .as_slice()
        .iter()
        .flat_map(|violation| violation.failures.clone())
        .collect();
    assert_eq!(
        failures,
        BTreeSet::from([
            GuardFailure::Unticked("request".parse().unwrap()),
            GuardFailure::Unticked("granted".parse().unwrap()),
        ])
    );

    let ticked = json!([
        { "op": "set_ticked", "node": "n_access", "item": "request", "ticked": true },
        { "op": "set_ticked", "node": "n_access", "item": "granted", "ticked": true },
        { "op": "transition", "node": "n_access", "transition": "complete" },
    ]);
    let applied = world
        .ok(
            &ann,
            "update_journey_state",
            state("p_access_ticked", 5, ticked),
        )
        .await;
    assert_eq!(applied["revision"], 6);
    let node = world
        .ok(
            &ann,
            "get_node",
            json!({ "journey": "j_vendor_eval", "node": "n_access" }),
        )
        .await;
    assert_eq!(node["detail"]["ticks"], json!(["granted", "request"]));
    assert_eq!(
        node["detail"]["derived"]["checklist_count"],
        json!({ "ticked": 2, "total": 2 })
    );
}

/// D4, D7: completing work whose relevance waits on an unanswered decision applies through
/// the tool, answered with the warning naming that decision.
#[tokio::test]
async fn completing_undecided_work_applies_with_a_warning() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let complete = json!([{ "op": "transition", "node": "n_baseline", "transition": "complete" }]);
    let complete = state("p_baseline_done", 1, complete);
    let applied = world.ok(&ann, "update_journey_state", complete).await;
    assert_eq!(applied["status"], "applied");
    assert_eq!(
        applied["consequences"]["j_vendor_eval"]["undecided"],
        json!([{ "node": "n_baseline", "unanswered": ["n_comparison_set"] }])
    );
}

/// Priority, D7, I5: answering a decision through the tool reports the informational half of
/// its consequences: what left scope, never a warning.
#[tokio::test]
async fn answering_a_decision_reports_what_left_scope() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let answer = json!([{ "op": "answer", "decision": "n_partner_runs",
        "value": { "boolean": false } }]);
    let answer = state("p_no_partner", 1, answer);
    let applied = world.ok(&ann, "update_journey_state", answer).await;
    let caused = &applied["consequences"]["j_vendor_eval"];
    assert_eq!(
        caused["out_of_scope"],
        json!(["n_criteria", "n_partner_led", "n_partner_results"])
    );
    assert_eq!(caused.get("undecided"), None, "no warning came with it");
}

/// H5: a write against a revision that moved is refused as stale with what moved; the same
/// patch id resubmitted after it landed is answered from its receipt.
#[tokio::test]
async fn stale_and_resubmitted_writes_are_answered_as_the_service_answers() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let answer = |patch_id: &str| {
        let purpose = json!([{ "op": "answer", "decision": "n_purpose",
            "value": { "single_choice": "purchase" } }]);
        state(patch_id, 1, purpose)
    };
    let applied = world
        .ok(&ann, "update_journey_state", answer("p_purpose"))
        .await;
    assert_eq!(applied["status"], "applied");
    assert_eq!(applied["revision"], 2);
    let again = world
        .ok(&ann, "update_journey_state", answer("p_purpose"))
        .await;
    assert_eq!(again["status"], "already_applied");
    assert_eq!(again["revision"], applied["revision"]);
    let mut keys: Vec<&str> = applied
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert!(
        keys.iter()
            .all(|key| ["consequences", "revision", "status"].contains(key)),
        "a write answers its revision and what it caused, not what it was sent: {keys:?}"
    );

    let stale = world
        .call(&ann, "update_journey_state", answer("p_purpose_again"))
        .await;
    let Err(stale_error) = stale else {
        panic!("{stale:?}");
    };
    let ToolError::Rejected {
        rejection: Rejection::Stale { conflicts, .. },
    } = &stale_error
    else {
        panic!("{stale_error:?}");
    };
    assert_eq!(
        (conflicts[0].expected.get(), conflicts[0].current.get()),
        (1, 2)
    );
    let served = serde_json::to_value(&stale_error).unwrap();
    let message = served["message"].as_str().unwrap();
    for named in ["j_vendor_eval", "2"] {
        assert!(message.contains(named), "{named} in {message}");
    }
    assert!(
        !message.contains("Domain(") && !message.contains("JourneyId"),
        "no debug form: {message}"
    );
}

/// B2: an answer takes a rationale; the node and the snapshot read it back.
#[tokio::test]
async fn an_answers_rationale_is_written_and_read_back() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let rationale = "- buy it\n- [quote](https://example.org)";
    let answer = json!([{ "op": "answer", "decision": "n_purpose",
        "value": { "single_choice": "purchase" }, "rationale": rationale }]);
    world
        .ok(&ann, "update_journey_state", state("p_why", 1, answer))
        .await;
    let node = json!({ "journey": "j_vendor_eval", "node": "n_purpose" });
    let node = world.ok(&ann, "get_node", node).await;
    assert_eq!(node["detail"]["rationale"], json!(rationale));
}

/// I7: an agent's write records the agent and the user it acts for, with its note.
#[tokio::test]
async fn an_agent_write_records_the_agent_and_its_user() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let helper = agent("ag_helper", "u_ann");
    let mut arguments = state(
        "p_kickoff",
        1,
        json!([{ "op": "transition", "node": "n_kickoff", "transition": "complete" }]),
    );
    arguments["note"] = json!("Kickoff held.");
    world.ok(&helper, "update_journey_state", arguments).await;
    let history = json!({ "journey": "j_vendor_eval", "node": "n_kickoff" });
    let history = world.ok(&ann, "get_history", history).await;
    let patches = history["patches"].as_array().unwrap();
    let last = patches.last().unwrap();
    assert_eq!(
        last["actor"],
        json!({ "user": "u_ann", "agent": "ag_helper" })
    );
    assert_eq!(last["note"], "Kickoff held.");
}

/// H5, E6: a merge resubmitted under its patch id is answered from its receipt even after a
/// journey it names has moved, so its journey list would now be drafted differently; the
/// same id asking for another merge is refused as reused.
#[tokio::test]
async fn a_merge_resubmitted_after_its_journeys_moved_is_answered_from_its_receipt() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let mut informed = state(
        "p_informed",
        1,
        json!([{ "op": "answer", "decision": "n_who_informed",
            "value": { "identity_list": ["who_d4m6uk2eqf5rbhzs", "who_e7h2jv3nxc4yaw6t"] } }]),
    );
    informed["deployment_revision"] = json!(1);
    world.ok(&ann, "update_journey_state", informed).await;
    let merge = |survivor: &str, merged: &str| {
        json!({ "change": { "merge": { "survivor": survivor, "merged": [merged] } },
            "patch_id": "p_merge", "base_revision": 1 })
    };
    let merged = world
        .ok(
            &ann,
            "manage_identity",
            merge("who_d4m6uk2eqf5rbhzs", "who_e7h2jv3nxc4yaw6t"),
        )
        .await;
    assert_eq!(merged["status"], "applied");

    let complete = json!([{ "op": "transition", "node": "n_kickoff", "transition": "complete" }]);
    world
        .ok(&ann, "update_journey_state", state("p_done", 2, complete))
        .await;
    let again = world
        .ok(
            &ann,
            "manage_identity",
            merge("who_d4m6uk2eqf5rbhzs", "who_e7h2jv3nxc4yaw6t"),
        )
        .await;
    assert_eq!(again["status"], "already_applied");
    assert_eq!(again["revision"], merged["revision"]);
    let other = world
        .call(
            &ann,
            "manage_identity",
            merge("who_k3d7qm2xw5tn6pfa", "who_d4m6uk2eqf5rbhzs"),
        )
        .await;
    let reused = matches!(
        other,
        Err(ToolError::Rejected {
            rejection: Rejection::PatchIdReused { .. }
        })
    );
    assert!(reused, "{other:?}");
}

/// A18: the removal `get_node` returns removes a node with a child, an incoming edge and a note
/// in one patch once the outside condition it names is cleared in that patch; the removal alone
/// is refused for the condition it leaves dangling.
#[tokio::test]
async fn the_removal_read_is_what_a_patch_needs_to_remove_a_node() {
    let world = World::new();
    let ann = user("u_ann");
    let nodes = [
        json!({ "key": "n_phase", "id": "phase", "kind": "group", "title": "Phase" }),
        json!({ "key": "n_ask", "id": "ask", "kind": "decision", "title": "Ask",
            "parent": "n_phase", "prompt": "Go?", "answer_type": "boolean" }),
        json!({ "key": "n_after", "id": "after", "kind": "action", "title": "After",
            "requires": ["n_phase"] }),
        json!({ "key": "n_extra", "id": "extra", "kind": "action", "title": "Extra",
            "relevant_when": { "equals": { "decision": "n_ask", "value": true } } }),
    ];
    let mut setup = vec![json!({ "op": "create_journey", "name": "Removal" })];
    setup.extend(nodes.map(|node| json!({ "op": "add_node", "node": node })));
    setup.push(json!({ "op": "add_annotation",
        "annotation": { "key": "a_note", "node": "n_phase", "note": "Seen." } }));
    let patch = |id: &str, base: u32, mutations: Vec<Value>| {
        json!({ "target": { "journey": "j_removal" }, "patch_id": id,
            "base_revision": base, "mutations": mutations })
    };
    let created = world
        .ok(&ann, "apply_patch", patch("p_setup", 0, setup))
        .await;
    let revision = u32::try_from(created["revision"].as_u64().unwrap()).unwrap();

    let read = json!({ "journey": "j_removal", "node": "n_phase", "removal": true });
    let read = world.ok(&ann, "get_node", read).await["removal"].clone();
    assert_eq!(read["removal"]["descendants"], json!(["n_ask"]));
    assert_eq!(read["removal"]["annotations"], json!(["a_note"]));
    assert_eq!(
        read["removal"]["edges"],
        json!([{ "node": "n_after", "requires": "n_phase" }])
    );
    assert_eq!(
        read["outside_references"],
        json!([{ "holder": "n_extra", "field": "relevant_when", "target": "n_ask" }])
    );

    let remove = json!({ "op": "remove_node", "removal": read["removal"] });
    let alone = patch("p_alone", revision, vec![remove.clone()]);
    let refused = world.call(&ann, "apply_patch", alone).await;
    assert!(
        matches!(refused, Err(ToolError::Rejected { .. })),
        "the removal alone leaves the condition dangling: {refused:?}"
    );

    let clear = json!({ "op": "set_node_field", "node": "n_extra",
        "value": { "relevant_when": null } });
    let both = patch("p_both", revision, vec![remove, clear]);
    world.ok(&ann, "apply_patch", both).await;
    let listed = world
        .ok(&ann, "list_nodes", json!({ "journey": "j_removal" }))
        .await;
    let mut left: Vec<&str> = listed["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["key"].as_str().unwrap())
        .collect();
    left.sort_unstable();
    assert_eq!(left, ["n_after", "n_extra"]);
}

/// A journey is created with owners, found by them, and named by them in the snapshot and
/// the index as the identities they are now; a create drafted against an older deployment
/// revision than a merge of its owner is stale.
#[tokio::test]
async fn a_journey_is_created_with_owners_and_a_merge_moves_them_on() {
    let world = World::new();
    let ann = user("u_ann");
    for (patch_id, key, base) in [("p_a", "who_a", 0), ("p_b", "who_b", 1)] {
        let create = json!({ "change": { "create": { "identity": { "key": key, "name": key } } },
            "patch_id": patch_id, "base_revision": base });
        world.ok(&ann, "manage_identity", create).await;
    }
    let deployment = world.service.deployment().await.unwrap().revision.get();
    let create = |journey: &str, owner: &str, deployment_revision: u32| {
        json!({ "target": { "journey": journey }, "patch_id": format!("p_{journey}"),
            "base_revision": 0, "deployment_revision": deployment_revision,
            "mutations": [{ "op": "create_journey", "name": journey, "owners": [owner] }] })
    };
    world
        .ok(&ann, "apply_patch", create("j_owned", "who_a", deployment))
        .await;
    let owners = |value: &serde_json::Value| value["owners"].clone();
    let owned = world
        .ok(&ann, "list_journeys", json!({ "owners": ["who_a"] }))
        .await;
    assert_eq!(owned["journeys"][0]["id"], "j_owned");
    assert_eq!(owners(&owned["journeys"][0]), json!(["who_a"]));
    let snapshot = world
        .ok(&ann, "get_snapshot", json!({ "journey": "j_owned" }))
        .await;
    assert_eq!(owners(&snapshot["snapshot"]["journey"]), json!(["who_a"]));

    let merge = json!({ "change": { "merge": { "survivor": "who_b", "merged": ["who_a"] } },
        "patch_id": "p_merge", "base_revision": deployment });
    world.ok(&ann, "manage_identity", merge).await;
    let merged = world
        .ok(&ann, "list_journeys", json!({ "owners": ["who_a"] }))
        .await;
    assert_eq!(
        merged["journeys"][0]["id"], "j_owned",
        "the merged key still finds it"
    );
    assert_eq!(owners(&merged["journeys"][0]), json!(["who_b"]));
    let snapshot = world
        .ok(&ann, "get_snapshot", json!({ "journey": "j_owned" }))
        .await;
    assert_eq!(owners(&snapshot["snapshot"]["journey"]), json!(["who_b"]));

    // The deployment revision the create names is checked: it moved on (the merge), and the
    // write, which touches nothing the merge did, lands over it and says so.
    let late = world
        .ok(&ann, "apply_patch", create("j_late", "who_b", deployment))
        .await;
    assert_eq!(
        late["over"]["conflicts"][0]["of"],
        json!({ "domain": "deployment" }),
        "{late}"
    );
    let snapshot = world
        .ok(&ann, "get_snapshot", json!({ "journey": "j_late" }))
        .await;
    assert_eq!(owners(&snapshot["snapshot"]["journey"]), json!(["who_b"]));
}

/// Starting work that waits is a plain transition: no guard and no bypass, and the node reads
/// active.
#[tokio::test]
async fn starting_work_that_waits_is_a_plain_transition() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let start = json!([{ "op": "transition", "node": "n_plan", "transition": "start" }]);
    let applied = world
        .ok(
            &ann,
            "update_journey_state",
            state("p_plan_start", 1, start),
        )
        .await;
    assert_eq!(applied["status"], "applied");
    let node = json!({ "journey": "j_vendor_eval", "node": "n_plan" });
    let node = world.ok(&ann, "get_node", node).await;
    assert_eq!(node["detail"]["derived"]["display_state"], "active");
}

/// `update_journey_state` records what happened in a journey and nothing else: a structural
/// op, or an identity edit (the deployment's), is refused at its path, pointing elsewhere, and
/// nothing is written.
#[tokio::test]
async fn journey_state_refuses_what_is_not_journey_state_by_its_path() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let start = json!({ "op": "transition", "node": "n_kickoff", "transition": "start" });
    let others = [
        json!({ "op": "set_node_field", "node": "n_plan", "value": { "estimate": 3 } }),
        json!({ "op": "add_edge", "edge": { "node": "n_plan", "requires": "n_kickoff" } }),
        json!({ "op": "edit_identity", "identity": { "key": "who_d4m6uk2eqf5rbhzs",
            "name": "Dara" } }),
    ];
    for other in others {
        let refused = world
            .call(
                &ann,
                "update_journey_state",
                state("p_mixed", 1, json!([start.clone(), other.clone()])),
            )
            .await;
        let Err(ToolError::Arguments { path, message }) = refused else {
            panic!("{other}: {refused:?}");
        };
        assert_eq!(path, "mutations[1].op", "{other}");
        assert!(message.contains("apply_patch"), "{message}");
    }
    let snapshot = world
        .ok(&ann, "get_snapshot", json!({ "journey": "j_vendor_eval" }))
        .await;
    assert_eq!(snapshot["revision"], 1, "nothing was written");
}

/// E6, B2: a person an answer names who is not in Cairn yet is created in the same patch as
/// the answer: one write, one journey revision, the answer naming the new identity.
#[tokio::test]
async fn a_new_person_rides_with_the_answer_that_names_them() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let deployment = world.service.deployment().await.unwrap().revision.get();
    let mut named = state(
        "p_new_owner",
        1,
        json!([
            { "op": "create_identity", "identity": { "key": "who_p9n2x7k4q8r3t5vw",
                "name": "Priya Nair", "emails": ["priya@example.org"] } },
            { "op": "answer", "decision": "n_who_owns",
                "value": { "identity": "who_p9n2x7k4q8r3t5vw" } },
        ]),
    );
    named["deployment_revision"] = json!(deployment);
    let applied = world.ok(&ann, "update_journey_state", named).await;
    assert_eq!(applied["revision"], 2);
    let node = json!({ "journey": "j_vendor_eval", "node": "n_who_owns" });
    let node = world.ok(&ann, "get_node", node).await;
    assert_eq!(
        node["detail"]["answer"],
        json!({ "identity": "who_p9n2x7k4q8r3t5vw" })
    );
    let found = world
        .ok(&ann, "list_identities", json!({ "text": "Priya" }))
        .await;
    assert_eq!(
        found["identities"]["items"][0]["key"],
        "who_p9n2x7k4q8r3t5vw"
    );
}

/// A18, A17, B5: a removal, the unsnooze it needs and a journey weight are one `apply_patch`:
/// one revision, the node gone, the snooze that named it lifted and the weight set. The
/// removal alone is refused for the snooze it leaves naming nothing.
#[tokio::test]
async fn a_removal_rides_with_the_state_it_needs_in_one_patch() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let snooze =
        json!([{ "op": "snooze", "node": "n_findings", "until": { "node": "n_baseline" } }]);
    world
        .ok(&ann, "update_journey_state", state("p_snooze", 1, snooze))
        .await;
    let read = json!({ "journey": "j_vendor_eval", "node": "n_baseline", "removal": true });
    let read = world.ok(&ann, "get_node", read).await["removal"].clone();
    let holders: Vec<&str> = read["outside_references"]
        .as_array()
        .unwrap()
        .iter()
        .map(|reference| reference["holder"].as_str().unwrap())
        .collect();
    assert!(holders.contains(&"n_findings"), "{read}");
    let remove = json!({ "op": "remove_node", "removal": read["removal"] });
    let alone = world
        .call(
            &ann,
            "apply_patch",
            patch("p_alone", 2, json!([remove.clone()])),
        )
        .await;
    assert!(
        matches!(alone, Err(ToolError::Rejected { .. })),
        "{alone:?}"
    );
    let together = json!([
        remove,
        { "op": "unsnooze", "node": "n_findings" },
        { "op": "set_node_field", "node": "n_plan", "value": { "weight": 3 } },
    ]);
    let applied = world
        .ok(&ann, "apply_patch", patch("p_together", 2, together))
        .await;
    assert_eq!(applied["revision"], 3);
    let gone = world
        .call(
            &ann,
            "get_node",
            json!({ "journey": "j_vendor_eval", "node": "n_baseline" }),
        )
        .await;
    assert!(matches!(gone, Err(ToolError::NotFound { .. })), "{gone:?}");
    let findings = json!({ "journey": "j_vendor_eval", "node": "n_findings" });
    let findings = world.ok(&ann, "get_node", findings).await;
    assert_eq!(findings["detail"]["derived"].get("snoozed"), None);
    let plan = json!({ "journey": "j_vendor_eval", "node": "n_plan" });
    let plan = world.ok(&ann, "get_node", plan).await;
    assert_eq!(plan["detail"]["node"]["weight"], 3);
}
