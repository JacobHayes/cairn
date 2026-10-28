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
    assert_eq!(created["proposal"], "pr_workload");
    assert_eq!(created["revision"], 1);
    assert_eq!(
        created["destination"],
        json!({ "journey": "j_vendor_eval" })
    );
    assert_eq!(
        created["destination_revision"],
        breakdown()["destination_revision"]
    );
    assert_eq!(created.get("draft"), None, "the draft is not echoed back");
    let held = world
        .ok(&ann, "get_proposal", json!({ "proposal": "pr_workload" }))
        .await;
    assert_eq!(held["proposal"]["proposing_agent"], "ag_helper");
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
    assert_eq!(applied["revision"], 2);
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
    assert_eq!(discarded["state"], "discarded");
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
    let free = json!({ "target": { "journey": "j_free" }, "patch_id": "p_free", "base_revision": 0,
        "mutations": [{ "op": "create_journey", "name": "Free" }] });
    world.ok(&ann, "apply_patch", free).await;
    let upgrade = |journey: &str| json!({ "upgrade": { "journey": journey, "to": 2 }, "proposal": "pr_up", "patch_id": "p_up" });
    let relink = json!({ "proposal": "pr_relink", "patch_id": "p_relink", "relink": {
        "journey": "j_vendor_eval", "to": { "route": "vendor-evaluation", "version": 9 } } });
    let cases = [
        ("upgrade", upgrade("j_missing"), "not found"),
        ("relink", relink, "not found"),
        ("upgrade", upgrade("j_free"), "refused"),
    ];
    for (tool, arguments, kind) in cases {
        let answered = world.call(&ann, "create_proposal", arguments).await;
        let found = match &answered {
            Err(cairn_mcp::ToolError::NotFound { .. }) => "not found",
            Err(cairn_mcp::ToolError::Refused { .. }) => "refused",
            _ => "something else",
        };
        assert_eq!(found, kind, "{tool}: {answered:?}");
    }
}

/// I6: `create_proposal` holds exactly one content, as its listing says: naming none is
/// refused, and naming two kinds, or half of your own draft, is refused at the argument that
/// makes it wrong; a content named as null is refused there too, not read as absent.
#[tokio::test]
async fn a_proposal_names_exactly_one_content() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let base = json!({ "proposal": "pr_one", "patch_id": "p_one" });
    let with = |extra: Value| {
        let mut arguments = base.clone();
        arguments
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        arguments
    };
    let relink =
        json!({ "journey": "j_vendor_eval", "to": { "route": "vendor-evaluation", "version": 1 } });
    let cases = [
        (base.clone(), ""),
        (
            with(json!({ "upgrade": { "journey": "j_vendor_eval", "to": 2 }, "relink": relink })),
            "relink",
        ),
        (
            with(json!({ "destination": { "journey": "j_vendor_eval" } })),
            "draft",
        ),
        (
            with(json!({ "draft": breakdown(), "relink": relink })),
            "relink",
        ),
        (
            with(json!({ "relink": relink, "upgrade": null })),
            "upgrade",
        ),
    ];
    let listed = cairn_mcp::ToolSet::definitions()
        .into_iter()
        .find(|tool| tool.name == "create_proposal")
        .unwrap();
    let listed = jsonschema::draft202012::new(&Value::Object(listed.input_schema)).unwrap();
    for (arguments, at) in cases {
        assert!(
            !listed.is_valid(&arguments),
            "the listing refuses {arguments}"
        );
        let answered = world.call(&ann, "create_proposal", arguments.clone()).await;
        let Err(cairn_mcp::ToolError::Arguments { path, .. }) = &answered else {
            panic!("{arguments}: {answered:?}");
        };
        assert_eq!(path, at, "{arguments}");
    }
    let listed = json!({ "scope": "destination", "destination": { "journey": "j_vendor_eval" } });
    let listed = world.ok(&ann, "list_proposals", listed).await;
    assert_eq!(listed["count"], 0, "nothing was drafted");
}
