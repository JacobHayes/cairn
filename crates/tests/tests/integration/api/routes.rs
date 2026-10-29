//! Route files and the drafting flows over HTTP, in process against the memory store
//! (PRACTICES, Shell: API tests): a version exported and imported back as a new draft (A13);
//! save as route drafted as a proposal (B8); and each request that cannot be drafted answers
//! its problem.
#![cfg(test)]

mod in_process {
    use axum::http::{Method, StatusCode};
    use cairn_api::client::Reply;
    use cairn_schema::{Domain, Proposal, ReviewItem, RouteFile, from_yaml};
    use cairn_service::wire::{PatchAnswer, Problem, ProblemCode, ProposalAnswer};
    use serde_json::json;

    use crate::api::support::{self, World, get, ok, post};

    const ROUTE: &str = "/api/routes/vendor-evaluation";
    const JOURNEY: &str = "/api/journeys/j_vendor_eval";

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
        // A20: the import lists what the final milestone cannot see, and still applies.
        let PatchAnswer::Applied { notices, .. } = &imported else {
            panic!("expected an applied import, got {imported:?}");
        };
        let paths: Vec<String> = notices.iter().map(|n| n.path.to_string()).collect();
        assert_eq!(paths, ["purpose", "setup/workload"]);
        let again: PatchAnswer = ok(&post(&ann, &target, &import).await);
        assert!(
            matches!(again, PatchAnswer::AlreadyApplied { .. }),
            "{again:?}"
        );
        let draft: RouteFile = get(&ann, &format!("{ROUTE}/export")).await;
        assert_eq!(draft, exported, "the draft is version 1, every key kept");
    }

    /// B8, I6: save as route is drafted as a proposal for the route, with a review item per
    /// node to exclude.
    #[tokio::test]
    async fn save_as_route_is_drafted_as_a_proposal() {
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
    }

    /// `/routes/new` is the app's new-route form, so no route can be called `new`: a route
    /// file naming it, a save as route to it, and the path to it are refused before anything is
    /// stored (A13, B8).
    #[tokio::test]
    async fn no_route_can_be_called_new() {
        let world = World::start().await;
        let ann = world.vendor_after(3).await;
        let mut file = serde_json::to_value(route_v2()).unwrap();
        file["route"] = json!("new");
        let import = json!({"patch_id": "p_import_new", "file": file});
        let save = json!({
            "patch_id": "p_save_new",
            "proposal": "pr_save_new",
            "route": "new",
            "name": "New",
        });
        for (target, body) in [
            ("/api/routes/new/import".to_owned(), import.clone()),
            (format!("{ROUTE}/import"), import),
            (format!("{JOURNEY}/save-as-route"), save),
        ] {
            let reply = post(&ann, &target, &body).await;
            assert!(reply.status.is_client_error(), "{target}: {}", reply.status);
        }
    }

    /// A version that does not exist is not found; a file of another route is refused.
    #[tokio::test]
    async fn each_request_that_cannot_be_drafted_answers_its_problem() {
        let world = World::start().await;
        let ann = world.vendor_after(1).await;
        let mut other = route_v2();
        other.route = "another-route".parse().unwrap();
        let import = json!({"patch_id": "p_import", "file": other});
        let cases = [(
            format!("{ROUTE}/import"),
            import,
            ProblemCode::TargetMismatch,
        )];
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
