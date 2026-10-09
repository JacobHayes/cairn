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
            { "op": "transition", "node": "n_kickoff", "transition": "complete" },
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
    // Kickoff reached and access started: the workload is open and its stage has opened.
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
        "journey": "j_vendor_eval", "node": "n_kickoff", "transition": "reach",
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
        "resolution": { "op": "transition", "node": "n_kickoff", "transition": "reach" },
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
        "value": { "entity_list": ["e_stakeholder_a", "e_stakeholder_b"] },
        "deployment_revision": 1, "patch_id": "p_informed", "base_revision": 1,
    });
    world.ok(&ann, "answer_decision", informed).await;
    let merge = |survivor: &str, merged: &str| {
        json!({ "change": { "merge": { "survivor": survivor, "merged": merged } },
            "patch_id": "p_merge", "base_revision": 1 })
    };
    let merged = world
        .ok(
            &ann,
            "manage_entity",
            merge("e_stakeholder_a", "e_stakeholder_b"),
        )
        .await;
    assert_eq!(merged["status"], "applied");

    let reach = json!({ "journey": "j_vendor_eval", "node": "n_kickoff", "transition": "reach",
        "patch_id": "p_reach", "base_revision": 2 });
    world.ok(&ann, "transition_node", reach).await;
    let again = world
        .ok(
            &ann,
            "manage_entity",
            merge("e_stakeholder_a", "e_stakeholder_b"),
        )
        .await;
    assert_eq!(again["status"], "already_applied");
    assert_eq!(again["receipt"], merged["receipt"]);
    let other = world
        .call(&ann, "manage_entity", merge("e_lead", "e_stakeholder_a"))
        .await;
    let reused = matches!(
        other,
        Err(ToolError::Rejected {
            rejection: Rejection::PatchIdReused { .. }
        })
    );
    assert!(reused, "{other:?}");
}
