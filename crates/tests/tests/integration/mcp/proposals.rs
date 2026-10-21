//! The proposal tools in process over the memory store (I6): a proposal created under the
//! agent's id, reviewed, and applied by its user, who is recorded as confirming it (H2); a
//! create resubmitted by id answers the proposal as it stands.
#![cfg(test)]

use crate::mcp::support;

use serde_json::{Value, json};

use support::{World, agent, user};

/// I6, H2: an agent drafts a breakdown, its user reviews and applies it.
#[tokio::test]
async fn a_proposal_is_drafted_reviewed_and_applied() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let helper = agent("ag_helper", "u_ann");
    let create = json!({ "proposal": "pr_workload", "destination": { "journey": "j_vendor_eval" },
        "draft": breakdown(), "patch_id": "p_propose" });
    let created = world.ok(&helper, "create_proposal", create.clone()).await;
    assert_eq!(created["status"], "saved");
    assert_eq!(created["proposal"]["proposing_agent"], "ag_helper");
    let again = world.ok(&helper, "create_proposal", create).await;
    assert_eq!(
        again["status"], "already_saved",
        "a resubmitted create is answered"
    );

    let review = json!({ "proposal": "pr_workload", "review": true });
    let review = world.ok(&ann, "get_proposal", review).await;
    assert_eq!(review["review"].get("violations"), None);
    let apply = json!({ "proposal": "pr_workload", "reviewed_revision": 1, "patch_id": "p_apply" });
    let applied = world.ok(&ann, "apply_proposal", apply).await;
    assert_eq!(applied["receipt"]["revision"], 2);
    let held = world
        .ok(&ann, "get_proposal", json!({ "proposal": "pr_workload" }))
        .await;
    assert_eq!(held["proposal"]["status"], "applied");
}

/// A proposal is discarded against its editing revision; a stale edit is refused.
#[tokio::test]
async fn a_proposal_is_discarded_against_its_revision() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let create = json!({ "proposal": "pr_workload", "destination": { "journey": "j_vendor_eval" },
        "draft": breakdown(), "patch_id": "p_propose" });
    world.ok(&ann, "create_proposal", create).await;
    let stale = json!({ "proposal": "pr_workload", "change": "discard", "base_revision": 7, "patch_id": "p_stale" });
    let stale = world.call(&ann, "edit_proposal", stale).await;
    assert!(
        matches!(stale, Err(cairn_mcp::ToolError::Rejected { .. })),
        "{stale:?}"
    );
    let discard = json!({ "proposal": "pr_workload", "change": "discard", "base_revision": 1, "patch_id": "p_discard" });
    let discarded = world.ok(&ann, "edit_proposal", discard).await;
    assert_eq!(discarded["proposal"]["status"], "discarded");
}

/// A breakdown of the workload placeholder into two sub-deliverables (B10), drafted at
/// the journey's revision 1.
fn breakdown() -> Value {
    let child = |slug: &str| {
        json!({ "op": "add_node", "node": {
            "key": format!("n_workload_{slug}"), "id": slug, "parent": "n_workload",
            "kind": "deliverable", "title": format!("The {slug} workload"),
        }})
    };
    json!({ "title": "Break the workload down", "destination_revision": 1,
        "mutations": [child("ingest"), child("query")] })
}

/// B7, B9, I6: a draft the service cannot make is answered by its kind, as every host answers
/// it: a journey or route version that does not exist is not found, and a journey that follows
/// no route cannot be upgraded, which the caller is told is refused.
#[tokio::test]
async fn a_draft_that_cannot_be_made_is_answered_by_its_kind() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let free = json!({ "journey": "j_free", "patch_id": "p_free", "name": "Free" });
    world.ok(&ann, "create_journey", free).await;
    let upgrade = |journey: &str| json!({ "journey": journey, "to": 2, "proposal": "pr_up", "patch_id": "p_up" });
    let relink = json!({ "journey": "j_vendor_eval", "proposal": "pr_relink", "patch_id": "p_relink",
        "to": { "route": "vendor-evaluation", "version": 9 } });
    let cases = [
        ("upgrade", upgrade("j_missing"), "not found"),
        ("relink", relink, "not found"),
        ("upgrade", upgrade("j_free"), "refused"),
    ];
    for (tool, arguments, kind) in cases {
        let answered = world.call(&ann, tool, arguments).await;
        let found = match &answered {
            Err(cairn_mcp::ToolError::NotFound { .. }) => "not found",
            Err(cairn_mcp::ToolError::Refused { .. }) => "refused",
            _ => "something else",
        };
        assert_eq!(found, kind, "{tool}: {answered:?}");
    }
}
