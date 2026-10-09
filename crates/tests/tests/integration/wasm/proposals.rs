//! Brief 5.7 (native half): the in-browser root drafts, previews, refreshes, and applies
//! proposals as the server's API does (B7, B8, B9, C14, I6), answering the API's JSON for
//! each, so proposal review runs on either host. The browser tests drive the same calls
//! through the wasm build.
#![cfg(test)]

use cairn_api::wire;
use cairn_schema::{Rejection, ReviewItem, RouteFile, from_yaml};
use cairn_wasm::{BrowserRoot, HostError, ProposalAnswer, ProposalReview, read_document};
use serde_json::json;

/// After every fixture scenario's last step.
const NOW: &str = "2026-10-12T12:00:00Z";

fn thrown(error: &str) -> HostError {
    serde_json::from_str(error).unwrap()
}

fn patch_id(name: &str) -> String {
    format!("p_{name}")
}

fn route_revision(root: &BrowserRoot, route: &str) -> u64 {
    let read: serde_json::Value = serde_json::from_str(&root.route(route).unwrap()).unwrap();
    read["revision"].as_u64().unwrap()
}

fn journey_revision(root: &BrowserRoot, journey: &str) -> u32 {
    read_document(&root.document(journey, NOW).unwrap())
        .unwrap()
        .journey
        .revision
        .get()
}

/// Publishes the vendor evaluation's version 2 (fixtures/README.md) as a person does: the
/// file imported as a draft, then published.
fn publish_version_two(root: &BrowserRoot) {
    let file: RouteFile = from_yaml(include_str!(
        "../../../../../fixtures/vendor-evaluation/route-v2.yaml"
    ))
    .unwrap();
    let import = json!({ "patch_id": patch_id("import_v2"), "file": file }).to_string();
    root.import_file_json(&import, NOW).unwrap();
    let publish = json!({
        "patch": {
            "id": patch_id("publish_v2"),
            "target": { "route": "vendor-evaluation" },
            "base_revision": route_revision(root, "vendor-evaluation"),
            "mutations": [{ "op": "publish_draft" }],
        },
    })
    .to_string();
    root.patch(&publish, NOW).unwrap();
}

fn saved(answer: &str) -> cairn_schema::Proposal {
    let parsed: wire::ProposalAnswer = serde_json::from_str(answer).unwrap();
    assert_eq!(
        answer,
        serde_json::to_string(&parsed).unwrap(),
        "the API's JSON"
    );
    match serde_json::from_str::<ProposalAnswer>(answer).unwrap() {
        ProposalAnswer::Saved { proposal, .. } | ProposalAnswer::Existing { proposal } => proposal,
        ProposalAnswer::AlreadySaved { .. } => panic!("a new write is saved"),
    }
}

fn review(root: &BrowserRoot, id: &str) -> ProposalReview {
    let answer = root.preview_proposal_json(id, NOW).unwrap();
    let parsed: wire::ProposalReview = serde_json::from_str(&answer).unwrap();
    assert_eq!(
        answer,
        serde_json::to_string(&parsed).unwrap(),
        "the API's JSON"
    );
    serde_json::from_str(&answer).unwrap()
}

fn apply(root: &BrowserRoot, id: &str, name: &str, reviewed: u32) -> Result<String, String> {
    let request = json!({ "patch_id": patch_id(name), "reviewed_revision": reviewed }).to_string();
    root.apply_proposal_json(id, &request, NOW)
}

/// B7, C14: the scenario journey upgraded to version 2 proposes its orphan, previews clean,
/// and applies, after which it follows version 2.
#[test]
fn an_upgrade_is_drafted_previewed_and_applied() {
    let root = BrowserRoot::seeded().unwrap();
    publish_version_two(&root);
    let request = json!({ "patch_id": patch_id("upgrade"), "proposal": "pr_upgrade", "to": 2 });
    let proposal = saved(
        &root
            .upgrade_json("j_vendor_eval", &request.to_string(), NOW)
            .unwrap(),
    );
    assert!(
        proposal
            .draft
            .items
            .as_slice()
            .iter()
            .any(|item| matches!(item, ReviewItem::Orphan { node, keep: true, .. } if node.to_string() == "n_workload"))
    );
    let reviewed = review(&root, "pr_upgrade");
    assert_eq!(reviewed.stale, None);
    assert!(reviewed.preview.violations.is_empty() && reviewed.preview.unresolved.is_empty());
    apply(
        &root,
        "pr_upgrade",
        "apply_upgrade",
        proposal.revision.get(),
    )
    .unwrap();
    let document = read_document(&root.document("j_vendor_eval", NOW).unwrap()).unwrap();
    assert_eq!(
        document
            .journey
            .header
            .lineage
            .map(|lineage| lineage.version.get()),
        Some(2)
    );
}

/// I6: a proposal whose journey moved previews stale, is refused at the revision it was
/// reviewed at, and applies only once refreshed and applied at its new revision.
#[test]
fn a_stale_proposal_applies_only_after_a_refresh_at_its_new_revision() {
    let root = BrowserRoot::seeded().unwrap();
    let base = journey_revision(&root, "j_vendor_eval");
    let draft = json!({
        "title": "A note on kickoff",
        "destination_revision": base,
        "mutations": [{
            "op": "add_annotation",
            "annotation": { "key": "a_proposed", "node": "n_kickoff", "note": "Proposed." },
        }],
    });
    let create = json!({ "patch_id": patch_id("create"), "id": "pr_note", "draft": draft });
    let created = saved(
        &root
            .propose(r#"{"journey":"j_vendor_eval"}"#, &create.to_string(), NOW)
            .unwrap(),
    );
    let moved = json!({
        "patch": {
            "id": patch_id("moved"),
            "target": { "journey": "j_vendor_eval" },
            "base_revision": base,
            "mutations": [{ "op": "edit_journey", "name": "Moved on" }],
        },
    });
    root.patch(&moved.to_string(), NOW).unwrap();

    let stale = review(&root, "pr_note").stale.expect("the journey moved");
    assert_eq!(stale.conflict.expected.get(), base);
    let refused =
        thrown(&apply(&root, "pr_note", "too_early", created.revision.get()).unwrap_err());
    assert!(matches!(
        refused,
        HostError::Rejected {
            rejection: Rejection::Stale { .. }
        }
    ));

    let step = json!({ "patch_id": patch_id("refresh"), "base_revision": created.revision });
    let refreshed = saved(
        &root
            .refresh_proposal_json("pr_note", &step.to_string(), NOW)
            .unwrap(),
    );
    assert!(refreshed.revision > created.revision);
    assert_eq!(review(&root, "pr_note").stale, None);
    let unreviewed =
        thrown(&apply(&root, "pr_note", "old_review", created.revision.get()).unwrap_err());
    assert!(
        matches!(unreviewed, HostError::Rejected { .. }),
        "the review it had is of a revision the proposal has left: {unreviewed:?}"
    );
    apply(&root, "pr_note", "reviewed_again", refreshed.revision.get()).unwrap();
}

/// B8, B9: the routeless journey saved as a route, published, and re-linked to it.
#[test]
fn a_journey_saved_as_a_route_is_relinked_to_its_version() {
    let root = BrowserRoot::seeded().unwrap();
    let save = json!({
        "patch_id": patch_id("save"),
        "proposal": "pr_save",
        "route": "bake-off-route",
        "name": "Bake-off",
    });
    let proposal = saved(
        &root
            .save_as_route_json("j_bakeoff", &save.to_string(), NOW)
            .unwrap(),
    );
    let mut draft = proposal.draft.clone();
    for item in draft.items.as_mut_slice() {
        if let ReviewItem::Participation { mapping, .. } = item {
            mapping.replace(cairn_schema::ParticipationMapping::Drop);
        }
    }
    let edit =
        json!({ "patch_id": patch_id("map"), "base_revision": proposal.revision, "draft": draft });
    let mapped = saved(
        &root
            .edit_proposal_json("pr_save", &edit.to_string(), NOW)
            .unwrap(),
    );
    let reviewed = review(&root, "pr_save");
    assert!(
        reviewed.preview.unresolved.is_empty(),
        "{:?}",
        reviewed.preview.unresolved
    );
    apply(&root, "pr_save", "apply_save", mapped.revision.get()).unwrap();
    let publish = json!({
        "patch": {
            "id": patch_id("publish_saved"),
            "target": { "route": "bake-off-route" },
            "base_revision": route_revision(&root, "bake-off-route"),
            "mutations": [{ "op": "publish_draft" }],
        },
    });
    root.patch(&publish.to_string(), NOW).unwrap();

    let relink = json!({
        "patch_id": patch_id("relink"),
        "proposal": "pr_relink",
        "lineage": { "route": "bake-off-route", "version": 1 },
    });
    let proposal = saved(
        &root
            .relink_json("j_bakeoff", &relink.to_string(), NOW)
            .unwrap(),
    );
    apply(&root, "pr_relink", "apply_relink", proposal.revision.get()).unwrap();
    let document = read_document(&root.document("j_bakeoff", NOW).unwrap()).unwrap();
    assert_eq!(
        document
            .journey
            .header
            .lineage
            .map(|lineage| lineage.route.to_string()),
        Some("bake-off-route".to_owned())
    );
}

/// What cannot be drafted is refused, not a panic: an upgrade of a journey with no route, a
/// proposal that does not exist.
#[test]
fn what_cannot_be_drafted_or_found_is_refused() {
    let root = BrowserRoot::seeded().unwrap();
    let request = json!({ "patch_id": patch_id("no_route"), "proposal": "pr_none", "to": 2 });
    let refused = thrown(
        &root
            .upgrade_json("j_bakeoff", &request.to_string(), NOW)
            .unwrap_err(),
    );
    assert!(matches!(refused, HostError::Failed { .. }), "{refused:?}");
    let missing = thrown(&root.proposal("pr_absent").unwrap_err());
    assert!(matches!(missing, HostError::Missing { .. }));
    let missing = thrown(&root.preview_proposal_json("pr_absent", NOW).unwrap_err());
    assert!(matches!(missing, HostError::Missing { .. }));
}
