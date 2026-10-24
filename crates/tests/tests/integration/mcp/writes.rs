//! The direct-write tools in process over the memory store: each is one patch, refused with
//! every violation (A15), answered stale with what moved or from its receipt when
//! resubmitted (H5), and recorded as the agent and the user it acts for (I7).
#![cfg(test)]

use crate::mcp::support;

use std::collections::BTreeSet;

use cairn_mcp::ToolError;
use cairn_schema::{Guard, GuardFailure, Rejection, Subject, ViolationCode};
use serde_json::json;

use support::{World, agent, user};

/// A15: a write with three independent violations is refused with all three, as the engine
/// reported them, and nothing is written.
#[tokio::test]
async fn a_refused_write_carries_every_violation() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let patch = json!({
        "id": "p_three",
        "target": { "journey": "j_vendor_eval" },
        "base_revision": 1,
        "mutations": [
            { "op": "transition", "node": "n_kickoff", "transition": "start" },
            { "op": "transition", "node": "n_nowhere", "transition": "start" },
            { "op": "answer", "decision": "n_partner_runs", "value": { "text": "maybe" } },
        ],
    });
    let refused = world
        .call(&ann, "apply_patch", json!({ "patch": patch }))
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

/// B10, A16, D4: completing a placeholder that is not broken down is refused through the
/// tool with the one guard that failed, named on the node and bypassable, and nothing is
/// written; the same completion bypassing that guard with a reason applies.
#[tokio::test]
async fn completing_a_placeholder_not_broken_down_is_refused_until_bypassed() {
    let world = World::new();
    let ann = user("u_ann");
    // Kickoff done and access started: the workload is open and its stage has opened.
    world.vendor_after(&ann, 4).await;
    let complete = json!({
        "journey": "j_vendor_eval", "node": "n_workload", "transition": "complete",
        "patch_id": "p_workload_done", "base_revision": 4,
    });
    let refused = world.call(&ann, "transition_node", complete).await;
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
    assert_eq!(violation.bypassable, Some(Guard::BrokenDown));
    let snapshot = json!({ "journey": "j_vendor_eval" });
    let snapshot = world.ok(&ann, "get_snapshot", snapshot).await;
    assert_eq!(snapshot["revision"], 4, "nothing was written");

    let bypassed = json!({
        "id": "p_workload_bypassed",
        "target": { "journey": "j_vendor_eval" },
        "base_revision": 4,
        "mutations": [
            { "op": "transition", "node": "n_workload", "transition": "complete" },
            { "op": "apply_override", "node": "n_workload", "override": { "guard_bypass": {
                "guards": ["broken_down"], "reason": "Run as one piece." } } },
        ],
    });
    let applied = world
        .ok(&ann, "apply_patch", json!({ "patch": bypassed }))
        .await;
    assert_eq!(applied["receipt"]["revision"], 5);
}

/// Gives the access node two items and the requirement to tick them, at revision 4.
async fn give_access_a_checklist(world: &World, ann: &cairn_schema::Actor) {
    let given = json!({
        "id": "p_items",
        "target": { "journey": "j_vendor_eval" },
        "base_revision": 4,
        "mutations": [
            { "op": "set_node_field", "node": "n_access", "value": { "checklist": [
                { "id": "request", "title": "Request sent" },
                { "id": "granted", "title": "Access granted" } ] } },
            { "op": "set_node_field", "node": "n_access", "value": { "requires_checklist": true } },
        ],
    });
    world
        .ok(ann, "apply_patch", json!({ "patch": given }))
        .await;
}

/// A16, D4, G5: work with a required checklist is refused through the tool once per unticked
/// item; the ticks and the completion sent as one patch apply, and the node reads back with its
/// ticks and count.
#[tokio::test]
async fn completing_work_with_a_checklist_needs_its_ticks_in_the_same_patch() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    world.vendor_after(&ann, 4).await;
    give_access_a_checklist(&world, &ann).await;
    let complete = json!({
        "journey": "j_vendor_eval", "node": "n_access", "transition": "complete",
        "patch_id": "p_access_done", "base_revision": 5,
    });
    let refused = world.call(&ann, "transition_node", complete).await;
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
    assert!(
        violations
            .as_slice()
            .iter()
            .all(|violation| violation.bypassable == Some(Guard::AllTicked))
    );

    let ticked = json!({
        "id": "p_access_ticked",
        "target": { "journey": "j_vendor_eval" },
        "base_revision": 5,
        "mutations": [
            { "op": "set_ticked", "node": "n_access", "item": "request", "ticked": true },
            { "op": "set_ticked", "node": "n_access", "item": "granted", "ticked": true },
            { "op": "transition", "node": "n_access", "transition": "complete" },
        ],
    });
    let applied = world
        .ok(&ann, "apply_patch", json!({ "patch": ticked }))
        .await;
    assert_eq!(applied["receipt"]["revision"], 6);
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
    let complete = json!({
        "journey": "j_vendor_eval", "node": "n_baseline", "transition": "complete",
        "patch_id": "p_baseline_done", "base_revision": 1,
    });
    let applied = world.ok(&ann, "transition_node", complete).await;
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
    let answer = json!({
        "journey": "j_vendor_eval", "decision": "n_partner_runs",
        "value": { "boolean": false }, "patch_id": "p_no_partner", "base_revision": 1,
    });
    let applied = world.ok(&ann, "answer_decision", answer).await;
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
        json!({
            "journey": "j_vendor_eval", "decision": "n_purpose",
            "value": { "single_choice": "purchase" },
            "patch_id": patch_id, "base_revision": 1,
        })
    };
    let applied = world.ok(&ann, "answer_decision", answer("p_purpose")).await;
    assert_eq!(applied["status"], "applied");
    assert_eq!(applied["receipt"]["revision"], 2);
    let again = world.ok(&ann, "answer_decision", answer("p_purpose")).await;
    assert_eq!(again["status"], "already_applied");
    assert_eq!(again["receipt"], applied["receipt"]);

    let stale = world
        .call(&ann, "answer_decision", answer("p_purpose_again"))
        .await;
    let Err(ToolError::Rejected {
        rejection: Rejection::Stale { conflicts, .. },
    }) = stale
    else {
        panic!("{stale:?}");
    };
    assert_eq!(
        (conflicts[0].expected.get(), conflicts[0].current.get()),
        (1, 2)
    );
}

/// B2: `answer_decision` takes a rationale; the node and the snapshot read it back.
#[tokio::test]
async fn an_answers_rationale_is_written_and_read_back() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let rationale = "- buy it\n- [quote](https://example.org)";
    let answer = json!({
        "journey": "j_vendor_eval", "decision": "n_purpose",
        "value": { "single_choice": "purchase" }, "rationale": rationale,
        "patch_id": "p_why", "base_revision": 1,
    });
    world.ok(&ann, "answer_decision", answer).await;
    let node = json!({ "journey": "j_vendor_eval", "node": "n_purpose" });
    let node = world.ok(&ann, "get_node", node).await;
    assert_eq!(node["detail"]["rationale"], json!(rationale));
    let snapshot = json!({ "journey": "j_vendor_eval" });
    let snapshot = world.ok(&ann, "get_snapshot", snapshot).await;
    assert_eq!(
        snapshot["snapshot"]["rationales"]["n_purpose"],
        json!(rationale)
    );
}

/// I7: an agent's write records the agent and the user it acts for, with its note.
#[tokio::test]
async fn an_agent_write_records_the_agent_and_its_user() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let helper = agent("ag_helper", "u_ann");
    let arguments = json!({
        "journey": "j_vendor_eval", "node": "n_kickoff", "transition": "complete",
        "patch_id": "p_kickoff", "base_revision": 1, "note": "Kickoff held.",
    });
    world.ok(&helper, "transition_node", arguments).await;
    let history = json!({ "journey": "j_vendor_eval", "node": "n_kickoff" });
    let history = world.ok(&ann, "get_history", history).await;
    let patches = history["patches"].as_array().unwrap();
    let last = &patches.last().unwrap()["events"][0];
    assert_eq!(
        last["actor"],
        json!({ "user": "u_ann", "agent": "ag_helper" })
    );
    assert_eq!(last["note"], "Kickoff held.");
}

/// A resolution move must be one a chain lists; anything else is refused before it is
/// written, naming the argument.
#[tokio::test]
async fn a_date_conflict_is_resolved_only_by_a_listed_move() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let not_a_move = json!({
        "journey": "j_vendor_eval",
        "resolution": { "op": "transition", "node": "n_kickoff", "transition": "complete" },
        "patch_id": "p_not_a_move", "base_revision": 1,
    });
    let refused = world.call(&ann, "resolve_date_conflict", not_a_move).await;
    assert!(matches!(refused, Err(ToolError::Arguments { path, .. }) if path == "resolution.op"));
}

/// H5, E6: a merge resubmitted under its patch id is answered from its receipt even after a
/// journey it names has moved, so its journey list would now be drafted differently; the
/// same id asking for another merge is refused as reused.
#[tokio::test]
async fn a_merge_resubmitted_after_its_journeys_moved_is_answered_from_its_receipt() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let informed = json!({
        "journey": "j_vendor_eval", "decision": "n_who_informed",
        "value": { "identity_list": ["who_d4m6uk2eqf5rbhzs", "who_e7h2jv3nxc4yaw6t"] },
        "deployment_revision": 1, "patch_id": "p_informed", "base_revision": 1,
    });
    world.ok(&ann, "answer_decision", informed).await;
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

    let complete = json!({ "journey": "j_vendor_eval", "node": "n_kickoff", "transition": "complete",
        "patch_id": "p_done", "base_revision": 2 });
    world.ok(&ann, "transition_node", complete).await;
    let again = world
        .ok(
            &ann,
            "manage_identity",
            merge("who_d4m6uk2eqf5rbhzs", "who_e7h2jv3nxc4yaw6t"),
        )
        .await;
    assert_eq!(again["status"], "already_applied");
    assert_eq!(again["receipt"], merged["receipt"]);
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
    let patch = |id: &str, base: u32, mutations: Vec<serde_json::Value>| {
        json!({ "patch": { "id": id, "target": { "journey": "j_removal" },
            "base_revision": base, "mutations": mutations } })
    };
    let created = world
        .ok(&ann, "apply_patch", patch("p_setup", 0, setup))
        .await;
    let revision = u32::try_from(created["receipt"]["revision"].as_u64().unwrap()).unwrap();

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
    let snapshot = world
        .ok(&ann, "get_snapshot", json!({ "journey": "j_removal" }))
        .await;
    let left: Vec<&str> = snapshot["snapshot"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["key"].as_str().unwrap())
        .collect();
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
        json!({ "journey": journey, "patch_id": format!("p_{journey}"), "name": journey,
            "owners": [owner], "deployment_revision": deployment_revision })
    };
    world
        .ok(
            &ann,
            "create_journey",
            create("j_owned", "who_a", deployment),
        )
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
        .ok(
            &ann,
            "create_journey",
            create("j_late", "who_b", deployment),
        )
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

/// Starting work that waits is refused through `transition_node` with a bypassable guard; the
/// bypass and the start in one `apply_patch` go through with no reason, and the argument
/// validates against the schema the tool advertises.
#[tokio::test]
async fn starting_ahead_takes_a_reasonless_bypass_in_one_patch() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let waiting = "n_plan";
    let start = json!({
        "journey": "j_vendor_eval", "node": waiting, "transition": "start",
        "patch_id": "p_plan_start", "base_revision": 1,
    });
    let refused = world.call(&ann, "transition_node", start).await;
    let Err(ToolError::Rejected {
        rejection: Rejection::Invalid { violations },
    }) = refused
    else {
        panic!("{refused:?}");
    };
    assert!(
        violations
            .as_slice()
            .iter()
            .all(|found| found.code == ViolationCode::GuardFailed
                && found.bypassable == Some(Guard::DepsDone)),
        "{violations:#?}"
    );
    let patch = json!({
        "id": "p_plan_ahead",
        "target": { "journey": "j_vendor_eval" },
        "base_revision": 1,
        "mutations": [
            { "op": "apply_override", "node": waiting, "override": { "guard_bypass": {
                "guards": ["deps_done"] } } },
            { "op": "transition", "node": waiting, "transition": "start" },
        ],
    });
    let arguments = json!({ "patch": patch });
    let advertised = cairn_mcp::ToolSet::definitions()
        .into_iter()
        .find(|tool| tool.name == "apply_patch")
        .unwrap();
    let validator =
        jsonschema::draft202012::new(&serde_json::Value::Object(advertised.input_schema)).unwrap();
    let errors: Vec<String> = validator
        .iter_errors(&arguments)
        .map(|error| error.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:#?}");
    let applied = world.ok(&ann, "apply_patch", arguments).await;
    assert_eq!(applied["receipt"]["revision"], 2);
}
