//! Route files and the drafting flows over HTTP, in process against the memory store
//! (PRACTICES, Shell: API tests): a version exported and imported back as a new draft (A13);
//! an upgrade drafted as a proposal whose conflict blocks its apply until resolved (B7, C14,
//! I6); save as route and re-link drafted as proposals (B8, B9); and each request that cannot
//! be drafted answers its problem.
#![cfg(test)]

mod in_process {
    use std::collections::BTreeSet;

    use axum::http::{Method, StatusCode};
    use cairn_api::client::{Reply, Transport};
    use cairn_api::wire::{PatchAnswer, Problem, ProblemCode, ProposalAnswer, ProposalReview};
    use cairn_schema::{
        Conflict, ConflictResolution, Domain, Journey, Kept, Proposal, Rejection, ReviewItem,
        Route, RouteFile, Subject, ViolationCode, from_yaml,
    };
    use serde_json::json;

    use crate::support::{self, World, get, ok, post};

    const ROUTE: &str = "/routes/vendor-evaluation";
    const JOURNEY: &str = "/journeys/j_vendor_eval";

    fn route_v2() -> RouteFile {
        let path = support::fixtures_root().join("vendor-evaluation/route-v2.yaml");
        from_yaml(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn saved(answer: ProposalAnswer) -> Proposal {
        match answer {
            ProposalAnswer::Saved { proposal, .. } => proposal,
            other => panic!("saved: {other:?}"),
        }
    }

    fn problem(reply: &Reply) -> (StatusCode, ProblemCode) {
        let problem: Problem = reply.json().unwrap();
        (reply.status, problem.error)
    }

    /// The journey after its first three steps with two local edits (B4), and version 2
    /// imported and published; answers ann's transport.
    async fn version_two_published(world: &World) -> Transport {
        let ann = world.vendor_after(3).await;
        let edits = support::patch(
            "p_local_edits",
            "{journey: j_vendor_eval}",
            3,
            "- op: set_node_field\n  node: n_access\n  value: {title: Access to the test environment}\n- op: set_node_field\n  node: n_findings\n  value: {estimate: 4}\n",
        );
        let target = format!("{JOURNEY}/patches");
        ok::<PatchAnswer>(&post(&ann, &target, &support::request(&edits)).await);
        let import = json!({"patch_id": "p_import_v2", "file": route_v2()});
        ok::<PatchAnswer>(&post(&ann, &format!("{ROUTE}/import"), &import).await);
        let route: Route = get(&ann, ROUTE).await;
        let publish = support::patch(
            "p_publish_v2",
            "{route: vendor-evaluation}",
            route.revision.get(),
            "- op: publish_draft\n",
        );
        let target = format!("{ROUTE}/patches");
        ok::<PatchAnswer>(&post(&ann, &target, &support::request(&publish)).await);
        ann
    }

    /// A13: version 1 exported names the version it extends; imported back under the same
    /// route it opens a draft whose export is the same file; resubmitted it is answered from
    /// its receipt.
    #[tokio::test]
    async fn an_export_imports_back_as_a_new_draft() {
        let world = World::start().await;
        let ann = world.vendor_after(1).await;
        let exported: RouteFile = get(&ann, &format!("{ROUTE}/export?version=1")).await;
        assert_eq!(
            exported.extends.map(cairn_schema::VersionNumber::get),
            Some(1)
        );
        let no_draft = ann
            .send(Method::GET, &format!("{ROUTE}/export"), None)
            .await;
        assert_eq!(
            problem(&no_draft.unwrap()),
            (StatusCode::NOT_FOUND, ProblemCode::NotFound)
        );

        let import = json!({"patch_id": "p_import", "file": exported});
        let target = format!("{ROUTE}/import");
        let imported: PatchAnswer = ok(&post(&ann, &target, &import).await);
        assert!(
            matches!(imported, PatchAnswer::Applied { .. }),
            "{imported:?}"
        );
        let again: PatchAnswer = ok(&post(&ann, &target, &import).await);
        assert!(
            matches!(again, PatchAnswer::AlreadyApplied { .. }),
            "{again:?}"
        );
        let draft: RouteFile = get(&ann, &format!("{ROUTE}/export")).await;
        assert_eq!(draft, exported, "the draft is version 1, every key kept");
    }

    /// B7, C14, I6: an upgrade is drafted as a proposal of review items, the journey keeps its
    /// version until it is applied, and its conflict blocks the apply until resolved.
    #[tokio::test]
    async fn an_upgrade_is_a_proposal_whose_conflict_blocks_until_resolved() {
        let world = World::start().await;
        let ann = version_two_published(&world).await;
        let upgrade = json!({"patch_id": "p_upgrade", "proposal": "pr_upgrade", "to": 2});
        let drafted = saved(ok(
            &post(&ann, &format!("{JOURNEY}/upgrade"), &upgrade).await
        ));
        // The title both sides changed conflicts, the estimate only the journey changed is
        // kept, and the workload version 2 removes is an orphan.
        let about: BTreeSet<(&str, &str)> = drafted
            .draft
            .items
            .as_slice()
            .iter()
            .map(|item| match item {
                ReviewItem::Conflict {
                    conflict: Conflict::Field { node, .. },
                    ..
                } => ("conflict", node.as_str()),
                ReviewItem::KeptLocalEdit {
                    kept: Kept::Node { node, .. },
                } => ("kept", node.as_str()),
                ReviewItem::Orphan { node, .. } => ("orphan", node.as_str()),
                other => panic!("{other:?}"),
            })
            .collect();
        let expected = [
            ("conflict", "n_access"),
            ("kept", "n_findings"),
            ("orphan", "n_workload"),
        ];
        assert_eq!(about, expected.into());
        let journey: Journey = get(&ann, JOURNEY).await;
        assert_eq!(journey.header.lineage.unwrap().version.get(), 1, "not yet");

        let review: ProposalReview =
            ok(&post(&ann, "/proposals/pr_upgrade/preview", &json!({})).await);
        assert_ne!(review.preview.unresolved.len(), 0);
        let apply = |patch_id: &str, reviewed: u32| json!({"patch_id": patch_id, "reviewed_revision": reviewed});
        let blocked = post(&ann, "/proposals/pr_upgrade/apply", &apply("p_blocked", 1)).await;
        assert_eq!(blocked.status, StatusCode::UNPROCESSABLE_ENTITY);
        let Rejection::Invalid { violations } = blocked.json().unwrap() else {
            panic!("an unresolved conflict blocks the apply")
        };
        let unresolved: Vec<&Option<Subject>> = violations
            .as_slice()
            .iter()
            .filter(|violation| violation.code == ViolationCode::UnresolvedReviewItem)
            .map(|violation| &violation.at.subject)
            .collect();
        let access = Subject::Node("n_access".parse().unwrap());
        assert_eq!(unresolved, [&Some(access)], "{violations:#?}");

        let mut resolved = drafted.draft.clone();
        for item in resolved.items.as_mut_slice() {
            if let ReviewItem::Conflict { resolution, .. } = item {
                *resolution = Some(ConflictResolution::TakeRoute);
            }
        }
        let edit = json!({"patch_id": "p_resolve", "base_revision": 1, "draft": resolved});
        let edited = ann
            .send(Method::PATCH, "/proposals/pr_upgrade", Some(&edit))
            .await;
        ok::<ProposalAnswer>(&edited.unwrap());
        let review: ProposalReview =
            ok(&post(&ann, "/proposals/pr_upgrade/preview", &json!({})).await);
        let left = (
            review.preview.unresolved.len(),
            review.preview.violations.len(),
        );
        assert_eq!(left, (0, 0));
        ok::<PatchAnswer>(&post(&ann, "/proposals/pr_upgrade/apply", &apply("p_apply", 2)).await);
        let journey: Journey = get(&ann, JOURNEY).await;
        assert_eq!(journey.header.lineage.unwrap().version.get(), 2);
    }

    /// B8, B9, I6: save as route is drafted as a proposal for the route, with a review item
    /// per node to exclude; re-link as one for the journey.
    #[tokio::test]
    async fn save_as_route_and_relink_are_drafted_as_proposals() {
        let world = World::start().await;
        let ann = world.vendor_after(3).await;
        let save = json!({
            "patch_id": "p_save",
            "proposal": "pr_save",
            "route": "saved-evaluation",
            "name": "Saved evaluation",
        });
        let drafted = saved(ok(
            &post(&ann, &format!("{JOURNEY}/save-as-route"), &save).await
        ));
        let route = Domain::Route("saved-evaluation".parse().unwrap());
        assert_eq!(drafted.destination, route);
        let exclusions = drafted.draft.items.as_slice().iter();
        let exclusions = exclusions.filter(|item| matches!(item, ReviewItem::Exclusion { .. }));
        assert_ne!(exclusions.count(), 0);

        let relink = json!({
            "patch_id": "p_relink",
            "proposal": "pr_relink",
            "lineage": {"route": "vendor-evaluation", "version": 1},
        });
        let drafted = saved(ok(&post(&ann, &format!("{JOURNEY}/relink"), &relink).await));
        let journey = Domain::Journey("j_vendor_eval".parse().unwrap());
        assert_eq!(drafted.destination, journey);
    }

    /// A journey or version that does not exist is not found; an upgrade of a journey that
    /// follows no route, or a file of another route, is refused.
    #[tokio::test]
    async fn each_request_that_cannot_be_drafted_answers_its_problem() {
        let world = World::start().await;
        let ann = world.vendor_after(1).await;
        let free = support::patch(
            "p_free",
            "{journey: j_free}",
            0,
            "- op: create_journey\n  name: Free\n",
        );
        let target = "/journeys/j_free/patches";
        ok::<PatchAnswer>(&post(&ann, target, &support::request(&free)).await);
        let upgrade =
            |to: u32| json!({"patch_id": format!("p_up_{to}"), "proposal": "pr_up", "to": to});
        let relink = json!({
            "patch_id": "p_relink",
            "proposal": "pr_relink",
            "lineage": {"route": "vendor-evaluation", "version": 9},
        });
        let mut other = route_v2();
        other.route = "another-route".parse().unwrap();
        let import = json!({"patch_id": "p_import", "file": other});
        let cases = [
            (
                "/journeys/j_free/upgrade".to_owned(),
                upgrade(2),
                ProblemCode::CannotDraft,
            ),
            (
                "/journeys/j_missing/upgrade".to_owned(),
                upgrade(2),
                ProblemCode::NotFound,
            ),
            (format!("{JOURNEY}/relink"), relink, ProblemCode::NotFound),
            (
                format!("{ROUTE}/import"),
                import,
                ProblemCode::TargetMismatch,
            ),
        ];
        for (target, body, code) in cases {
            let reply = post(&ann, &target, &body).await;
            let expected = (cairn_api::error::status_of(code), code);
            assert_eq!(problem(&reply), expected, "{target}: {body}");
        }
        let missing = ann
            .send(Method::GET, &format!("{ROUTE}/export?version=9"), None)
            .await;
        assert_eq!(
            problem(&missing.unwrap()),
            (StatusCode::NOT_FOUND, ProblemCode::NotFound)
        );
    }
}
