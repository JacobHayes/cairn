//! The proposal tools in process over the memory store (I6): a proposal created under the
//! agent's id, reviewed, and applied by its user, who is recorded as confirming it (H2); a
//! create resubmitted by id answers the proposal as it stands.
#![cfg(test)]

use crate::mcp::support;

use serde_json::{Value, json};

use support::{World, agent, user};

/// `apply_patch` arguments writing `mutations` into proposal `pr_workload`, titled by the
/// note, drafted at the journey's revision 1.
fn proposing(patch_id: &str, mutations: &[Value]) -> Value {
    json!({ "target": { "journey": "j_vendor_eval" }, "base_revision": 1, "patch_id": patch_id,
        "note": "Break the workload down", "mutations": mutations, "proposal": "pr_workload" })
}

/// I6, H2: an agent drafts a breakdown into a proposal, adds to it, and its user reviews and
/// applies it; a resubmitted create is answered from its receipt.
#[tokio::test]
async fn a_proposal_is_drafted_extended_reviewed_and_applied() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let helper = agent("ag_helper", "u_ann");
    let [ingest, query] = breakdown();
    let create = proposing("p_propose", &[ingest]);
    let created = world.ok(&helper, "apply_patch", create.clone()).await;
    assert_eq!(created["status"], "saved");
    assert_eq!(created["proposal"], "pr_workload");
    assert_eq!(created["revision"], 1);
    assert_eq!(
        created["destination"],
        json!({ "journey": "j_vendor_eval" })
    );
    assert_eq!(created["destination_revision"], 1);
    let held = world
        .ok(&ann, "get_proposal", json!({ "proposal": "pr_workload" }))
        .await;
    assert_eq!(held["proposal"]["proposing_agent"], "ag_helper");
    assert_eq!(
        held["proposal"]["draft"]["title"],
        "Break the workload down"
    );
    let again = world.ok(&helper, "apply_patch", create.clone()).await;
    assert_eq!(
        again["status"], "already_saved",
        "a resubmitted create is answered"
    );
    let snapshot = world
        .ok(&ann, "get_snapshot", json!({ "journey": "j_vendor_eval" }))
        .await;
    assert_eq!(snapshot["revision"], 1, "drafting moves nothing");

    let mut extend = proposing("p_extend", &[query]);
    extend["proposal_revision"] = json!(1);
    let extended = world.ok(&helper, "apply_patch", extend.clone()).await;
    assert_eq!(
        (&extended["status"], &extended["revision"]),
        (&json!("saved"), &json!(2))
    );
    let resent = world.ok(&helper, "apply_patch", extend).await;
    assert_eq!(resent["status"], "already_saved");
    let replayed = world.ok(&helper, "apply_patch", create).await;
    assert_eq!(
        replayed["status"], "already_saved",
        "a lost create's resubmission is answered after the proposal has grown"
    );

    let review = json!({ "proposal": "pr_workload", "review": true });
    let review = world.ok(&ann, "get_proposal", review).await;
    assert_eq!(review["review"].get("violations"), None);
    assert_eq!(
        review["proposal"]["draft"]["mutations"]
            .as_array()
            .map(Vec::len),
        Some(2),
        "both writes are in the proposal"
    );
    let apply = json!({ "proposal": "pr_workload", "reviewed_revision": 2, "patch_id": "p_apply" });
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
    let [ingest, _] = breakdown();
    world
        .ok(&ann, "apply_patch", proposing("p_propose", &[ingest]))
        .await;
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

/// A breakdown of the workload placeholder into two sub-deliverables (B10).
fn breakdown() -> [Value; 2] {
    let child = |slug: &str| {
        json!({ "op": "add_node", "node": {
            "key": format!("n_workload_{slug}"), "id": slug, "parent": "n_workload",
            "kind": "deliverable", "title": format!("The {slug} workload"),
        }})
    };
    [child("ingest"), child("query")]
}

/// B8, I6: a save the service cannot make is answered by its kind, as every host answers it:
/// a save of a journey that does not exist is not found.
#[tokio::test]
async fn a_save_that_cannot_be_made_is_answered_by_its_kind() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let save = json!({ "target": { "route": "saved" }, "base_revision": 0, "patch_id": "p_save",
        "proposal": "pr_save", "mutations": [
            { "op": "create_route", "name": "Saved" },
            { "op": "open_draft", "source": { "save_as_route": { "journey": "j_missing" } } },
        ] });
    let answered = world.call(&ann, "apply_patch", save).await;
    assert!(
        matches!(answered, Err(cairn_mcp::ToolError::NotFound { .. })),
        "{answered:?}"
    );
}

/// I6: what makes a write to a proposal malformed is refused at the argument at fault, and
/// nothing is drafted or applied: a revision without a proposal, a new proposal with no title,
/// an existing one named without its revision, a deployment revision, a save beside other
/// content, and anything added to a save.
#[tokio::test]
async fn a_write_to_a_proposal_is_refused_at_the_argument_at_fault() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let [ingest, _] = breakdown();
    let with = |change: &dyn Fn(&mut Value)| {
        let mut arguments = proposing("p_one", std::slice::from_ref(&ingest));
        change(&mut arguments);
        arguments
    };
    let cases = [
        (
            with(&|arguments| {
                arguments.as_object_mut().unwrap().remove("proposal");
                arguments["proposal_revision"] = json!(1);
            }),
            "proposal_revision",
        ),
        (
            with(&|arguments| {
                arguments.as_object_mut().unwrap().remove("note");
            }),
            "note",
        ),
        (
            with(&|arguments| arguments["proposal_revision"] = json!(1)),
            "proposal_revision",
        ),
        (
            with(&|arguments| arguments["deployment_revision"] = json!(1)),
            "deployment_revision",
        ),
        (
            with(&|arguments| {
                arguments["target"] = json!({ "route": "saved" });
                arguments["mutations"] = json!([
                    { "op": "create_route", "name": "Saved" },
                    { "op": "open_draft", "source": { "save_as_route": { "journey": "j_vendor_eval" } } },
                    ingest.clone(),
                ]);
            }),
            "mutations",
        ),
    ];
    for (arguments, at) in cases {
        let answered = world.call(&ann, "apply_patch", arguments.clone()).await;
        let Err(cairn_mcp::ToolError::Arguments { path, .. }) = &answered else {
            panic!("{arguments}: {answered:?}");
        };
        assert_eq!(path, at, "{arguments}");
    }
    let listed = json!({ "scope": "destination", "destination": { "journey": "j_vendor_eval" } });
    let listed = world.ok(&ann, "list_proposals", listed).await;
    assert_eq!(listed["count"], 0, "nothing was drafted");
}

/// I6: an addition to a proposal drafted against another revision of its destination, or to
/// one that is closed, is rejected.
#[tokio::test]
async fn a_stale_or_closed_proposal_takes_no_addition() {
    let world = World::new();
    let ann = user("u_ann");
    world.vendor_journey(&ann).await;
    let [ingest, query] = breakdown();
    world
        .ok(&ann, "apply_patch", proposing("p_made", &[ingest]))
        .await;
    let mut stale = proposing("p_stale", std::slice::from_ref(&query));
    stale["proposal_revision"] = json!(1);
    stale["base_revision"] = json!(0);
    let answered = world.call(&ann, "apply_patch", stale).await;
    assert!(
        matches!(answered, Err(cairn_mcp::ToolError::Rejected { .. })),
        "{answered:?}"
    );
    let discard = json!({ "proposal": "pr_workload", "change": "discard", "base_revision": 1, "patch_id": "p_discard" });
    world.ok(&ann, "edit_proposal", discard).await;
    let mut closed = proposing("p_closed", &[query]);
    closed["proposal_revision"] = json!(2);
    let answered = world.call(&ann, "apply_patch", closed).await;
    assert!(
        matches!(answered, Err(cairn_mcp::ToolError::Rejected { .. })),
        "{answered:?}"
    );
}
