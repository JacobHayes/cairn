//! Proposals over HTTP, in process against the memory store (PRACTICES, Shell: API tests):
//! created by an agent under its own id and fetched rather than duplicated when resubmitted,
//! edited against their editing revision without moving the journey, previewed, and applied
//! by a reviewer recorded as confirming them (I6, H2, H5, C14); an apply against a moved
//! journey is stale with what intervened until the proposal is refreshed (I6); each malformed
//! proposal request answers its problem.
#![cfg(test)]

mod in_process {
    use axum::http::{Method, StatusCode};
    use cairn_api::client::{Reply, Transport};
    use cairn_api::wire::{
        Caller, EventPage, MintedToken, PatchAnswer, Problem, ProblemCode, ProposalAnswer,
        ProposalReview,
    };
    use cairn_schema::{Proposal, ProposalStatus, Rejection, RevisionOf};
    use serde_json::{Value, json};

    use crate::api::support::{World, get, ok, post};

    const JOURNEY: &str = "/api/journeys/j_vendor_eval";
    const PR_NOTE: &str = "/api/journeys/j_vendor_eval/proposals/pr_note";

    /// A draft adding note `key` to the journey, drafted at `revision`.
    fn note_draft(title: &str, key: &str, revision: u32) -> Value {
        json!({
            "title": title,
            "destination_revision": revision,
            "mutations": [{"op": "add_annotation", "annotation": {"key": key, "note": "Seen."}}],
        })
    }

    async fn patch(transport: &Transport, target: &str, body: &Value) -> Reply {
        transport
            .send(Method::PATCH, target, Some(body))
            .await
            .unwrap()
    }

    fn saved(answer: ProposalAnswer) -> Proposal {
        match answer {
            ProposalAnswer::Saved { proposal, .. } => proposal,
            other => panic!("saved: {other:?}"),
        }
    }

    fn rejection(reply: &Reply) -> Rejection {
        assert_eq!(reply.status, StatusCode::CONFLICT, "{reply:?}");
        reply.json().unwrap()
    }

    /// An agent token of `member`'s, as a signed-in transport.
    async fn agent_of(world: &World, member: &str) -> Transport {
        let named = json!({"name": "helper"});
        let minted: MintedToken =
            ok(&post(&world.signed_in(member), "/api/users/me/tokens", &named).await);
        world.bearer(&minted.token)
    }

    fn create_body(patch_id: &str) -> Value {
        json!({"patch_id": patch_id, "id": "pr_note", "draft": note_draft("Note", "a_note", 3)})
    }

    fn edit_body(patch_id: &str, base: u32) -> Value {
        json!({"patch_id": patch_id, "base_revision": base, "draft": note_draft("A note", "a_note", 3)})
    }

    fn apply_body(patch_id: &str, reviewed: u32) -> Value {
        json!({"patch_id": patch_id, "reviewed_revision": reviewed})
    }

    /// The vendor evaluation after its first three steps, and `pr_note` created by an agent
    /// of ann's and edited once, to revision 2; answers the agent.
    async fn drafted_by_agent(world: &World) -> Transport {
        world.vendor_after(3).await;
        let agent = agent_of(world, "ann").await;
        let proposals = format!("{JOURNEY}/proposals");
        ok::<ProposalAnswer>(&post(&agent, &proposals, &create_body("p_pr")).await);
        let edited = patch(&agent, PR_NOTE, &edit_body("p_edit", 1)).await;
        ok::<ProposalAnswer>(&edited);
        agent
    }

    /// I6, H5: a create resubmitted under the proposal's id answers the proposal, not a second
    /// one; under its patch id, its receipt; an edit at an old revision is stale; and drafting
    /// never moves the journey.
    #[tokio::test]
    async fn a_proposal_resubmitted_by_id_is_fetched_not_duplicated() {
        let world = World::start().await;
        let agent = drafted_by_agent(&world).await;
        let held: Proposal = get(&agent, PR_NOTE).await;
        assert_eq!(
            (held.revision.get(), held.status),
            (2, ProposalStatus::Open)
        );
        assert!(held.proposing_agent.is_some(), "the agent is recorded (I6)");

        let proposals = format!("{JOURNEY}/proposals");
        let lost: ProposalAnswer = ok(&post(&agent, &proposals, &create_body("p_pr_lost")).await);
        assert_eq!(lost, ProposalAnswer::Existing { proposal: held });
        let repeated: ProposalAnswer = ok(&post(&agent, &proposals, &create_body("p_pr")).await);
        assert!(
            matches!(repeated, ProposalAnswer::AlreadySaved { .. }),
            "{repeated:?}"
        );
        let old = patch(&agent, PR_NOTE, &edit_body("p_edit_old", 1)).await;
        let Rejection::Stale { conflicts, .. } = rejection(&old) else {
            panic!("an edit at an old revision is stale")
        };
        assert_eq!(
            conflicts[0].of,
            RevisionOf::Proposal("pr_note".parse().unwrap())
        );
        assert_eq!(world.revision("j_vendor_eval").await, 3, "H5");
    }

    /// I6, H2, H5, C14: a reviewer previews the agent's proposal and applies the revision they
    /// reviewed (an older one is stale), and is recorded as confirming every event; the
    /// apply resubmitted after a lost response is answered from its receipt.
    #[tokio::test]
    async fn a_reviewer_applies_a_proposal_and_is_recorded_as_confirming_it() {
        let world = World::start().await;
        drafted_by_agent(&world).await;
        let bob = world.signed_in("bob");
        let review: ProposalReview =
            ok(&post(&bob, &format!("{PR_NOTE}/preview"), &json!({})).await);
        assert_eq!(review.stale, None);
        assert_eq!(
            (
                review.preview.unresolved.len(),
                review.preview.violations.len()
            ),
            (0, 0)
        );
        let old = post(
            &bob,
            &format!("{PR_NOTE}/apply"),
            &apply_body("p_apply_old", 1),
        )
        .await;
        let Rejection::Stale { conflicts, .. } = rejection(&old) else {
            panic!("an apply of an older reviewed revision is stale")
        };
        assert_eq!(
            conflicts[0].of,
            RevisionOf::Proposal("pr_note".parse().unwrap())
        );
        let applied: PatchAnswer =
            ok(&post(&bob, &format!("{PR_NOTE}/apply"), &apply_body("p_apply", 2)).await);
        assert_eq!(applied.receipt().revision.get(), 4);
        let resubmitted = post(&bob, &format!("{PR_NOTE}/apply"), &apply_body("p_apply", 2)).await;
        let resubmitted: PatchAnswer = ok(&resubmitted);
        let receipt = applied.receipt().clone();
        assert_eq!(resubmitted, PatchAnswer::AlreadyApplied { receipt }, "H5");
        let held: Proposal = get(&bob, PR_NOTE).await;
        assert_eq!(held.status, ProposalStatus::Applied);
        let reviewer: Caller = get(&bob, "/api/users/me").await;
        let events: EventPage = get(&bob, "/api/events?patch=p_apply").await;
        assert_ne!(events.items.len(), 0);
        for logged in events.items {
            assert_eq!(
                logged.event.confirming_user.as_ref(),
                Some(&reviewer.user),
                "H2"
            );
        }
    }

    /// I6, C14, D7: a proposal can be created for a journey, a route that does not exist yet,
    /// or the deployment, and each previews clean; the journey's preview names what applying
    /// it would newly cause.
    #[tokio::test]
    async fn proposals_for_every_domain_are_created_and_previewed() {
        let world = World::start().await;
        let ann = world.vendor_after(3).await;
        for (_, target, id, draft) in crate::api::support::proposals_of_every_domain(&ann).await {
            let create = json!({"patch_id": format!("p_{id}"), "id": id, "draft": draft});
            let created = saved(ok(&post(&ann, &target, &create).await));
            let review: ProposalReview =
                ok(&post(&ann, &format!("{target}/{id}/preview"), &json!({})).await);
            assert_eq!(review.proposal, created, "{id}");
            let left = (
                review.preview.unresolved.len(),
                review.preview.violations.len(),
            );
            assert_eq!(left, (0, 0), "{id}");
            if id == "pr_charter" {
                let caused = &review.consequences[&"j_vendor_eval".parse().unwrap()];
                let stale: Vec<String> = caused
                    .stale
                    .iter()
                    .map(|found| found.node.to_string())
                    .collect();
                assert_eq!(
                    stale,
                    ["n_kickoff"],
                    "kickoff, done, now waits on the charter"
                );
            } else {
                assert_eq!(review.consequences.len(), 0, "{id}");
            }
        }
    }

    /// I6: a proposal discarded against its revision is no longer open.
    #[tokio::test]
    async fn a_discarded_proposal_is_closed() {
        let world = World::start().await;
        let ann = world.vendor_after(3).await;
        let create = create_body("p_pr");
        ok::<ProposalAnswer>(&post(&ann, &format!("{JOURNEY}/proposals"), &create).await);
        let discard = json!({"patch_id": "p_discard", "base_revision": 1});
        let discarded: ProposalAnswer =
            ok(&post(&ann, &format!("{PR_NOTE}/discard"), &discard).await);
        let ProposalAnswer::Saved { proposal, .. } = discarded else {
            panic!("saved: {discarded:?}")
        };
        assert_eq!(proposal.status, ProposalStatus::Discarded);
    }

    /// I6: once the journey moves past the revision a proposal was drafted against, its
    /// preview says what moved and its apply is stale with the intervening touched set; once
    /// refreshed and reviewed again, it applies.
    #[tokio::test]
    async fn an_apply_against_a_moved_journey_is_stale_until_refreshed() {
        let world = World::start().await;
        let ann = world.vendor_after(3).await;
        let create = create_body("p_pr");
        ok::<ProposalAnswer>(&post(&ann, &format!("{JOURNEY}/proposals"), &create).await);
        let other = crate::api::support::patch(
            "p_other",
            "{journey: j_vendor_eval}",
            3,
            "- op: add_annotation\n  annotation: {key: a_other, note: Meanwhile.}\n",
        );
        ok::<PatchAnswer>(
            &post(
                &ann,
                &format!("{JOURNEY}/patches"),
                &crate::api::support::request(&other),
            )
            .await,
        );

        let review: ProposalReview =
            ok(&post(&ann, &format!("{PR_NOTE}/preview"), &json!({})).await);
        let stale = review.stale.expect("the journey moved");
        assert_eq!(
            (stale.conflict.expected.get(), stale.conflict.current.get()),
            (3, 4)
        );
        assert!(!stale.intervening.is_empty());
        let Rejection::Stale {
            conflicts,
            intervening,
            ..
        } = rejection(
            &post(
                &ann,
                &format!("{PR_NOTE}/apply"),
                &apply_body("p_apply_stale", 1),
            )
            .await,
        )
        else {
            panic!("an apply against a moved journey is stale")
        };
        assert_eq!(conflicts, vec![stale.conflict.clone()]);
        assert_eq!(
            intervening, stale.intervening,
            "what intervened, as the preview showed"
        );

        let refresh = json!({"patch_id": "p_refresh", "base_revision": 1});
        let refreshed: ProposalAnswer =
            ok(&post(&ann, &format!("{PR_NOTE}/refresh"), &refresh).await);
        let ProposalAnswer::Saved { proposal, .. } = refreshed else {
            panic!("saved: {refreshed:?}")
        };
        assert_eq!(
            (
                proposal.revision.get(),
                proposal.draft.destination_revision.get()
            ),
            (2, 4)
        );
        let review: ProposalReview =
            ok(&post(&ann, &format!("{PR_NOTE}/preview"), &json!({})).await);
        assert_eq!(review.stale, None);
        let applied: PatchAnswer =
            ok(&post(&ann, &format!("{PR_NOTE}/apply"), &apply_body("p_apply", 2)).await);
        assert_eq!(applied.receipt().revision.get(), 5);
    }

    /// C14, C16: an open proposal is listed under its destination and, with the viewer's
    /// own, on their waiting list, until it is closed; another destination's proposal is not
    /// found under this one, for a read and for a write.
    #[tokio::test]
    async fn open_proposals_are_listed_under_their_destination_and_nowhere_else() {
        use cairn_api::wire::ProposalList;

        let world = World::start().await;
        let agent = drafted_by_agent(&world).await;
        let ann = world.signed_in("ann");
        let listed: ProposalList = get(&ann, &format!("{JOURNEY}/proposals")).await;
        let ids: Vec<_> = listed
            .items
            .iter()
            .map(|item| item.id.to_string())
            .collect();
        assert_eq!((listed.count, ids), (1, vec!["pr_note".to_owned()]));
        let now = listed
            .destination
            .expect("a destination listing says where");
        assert!(now.exists);
        assert_eq!(listed.items[0].now, now);

        let elsewhere: ProposalList = get(&ann, "/api/routes/vendor-evaluation/proposals").await;
        assert_eq!(elsewhere.count, 0);
        let waiting: ProposalList = get(&agent, "/api/proposals?scope=waiting").await;
        assert_eq!(waiting.count, 1, "the proposal the viewer's agent drafted");
        let nobody = world.signed_in("bob");
        let waiting: ProposalList = get(&nobody, "/api/proposals?scope=waiting").await;
        assert_eq!(waiting.count, 0, "not another user's");

        let wrong = "/api/routes/vendor-evaluation/proposals/pr_note";
        let read = ann.send(Method::GET, wrong, None).await.unwrap();
        assert_eq!(read.status, StatusCode::NOT_FOUND);
        let apply = post(&ann, &format!("{wrong}/apply"), &apply_body("p_wrong", 2)).await;
        assert_eq!(apply.status, StatusCode::NOT_FOUND);

        let discard = json!({"patch_id": "p_close", "base_revision": 2});
        ok::<ProposalAnswer>(&post(&ann, &format!("{PR_NOTE}/discard"), &discard).await);
        let listed: ProposalList = get(&ann, &format!("{JOURNEY}/proposals")).await;
        assert_eq!(listed.count, 0, "a closed proposal is history, not listed");
    }

    /// C14, C16: `scope=new_journeys` and `scope=new_routes` list the open proposals that
    /// would create a journey or a route, and none whose destination exists.
    #[tokio::test]
    async fn the_proposals_creating_a_journey_or_a_route_are_listed_by_kind() {
        use cairn_api::wire::ProposalList;

        let world = World::start().await;
        let ann = world.vendor_after(3).await;
        let mut created = crate::api::support::proposals_of_every_domain(&ann).await;
        let journey = json!({
            "title": "A new journey",
            "destination_revision": 0,
            "mutations": [{"op": "create_journey", "name": "New journey"}],
        });
        created.push((
            &cairn_api::endpoints::PROPOSE_JOURNEY,
            "/api/journeys/j_new_one/proposals".to_owned(),
            "pr_journey",
            journey,
        ));
        for (_, target, id, draft) in created {
            let create = json!({"patch_id": format!("p_{id}"), "id": id, "draft": draft});
            ok::<ProposalAnswer>(&post(&ann, &target, &create).await);
        }
        for (scope, expected) in [("new_journeys", "pr_journey"), ("new_routes", "pr_route")] {
            let listed: ProposalList = get(&ann, &format!("/api/proposals?scope={scope}")).await;
            let ids: Vec<_> = listed
                .items
                .iter()
                .map(|item| item.id.to_string())
                .collect();
            assert_eq!(
                (listed.count, ids),
                (1, vec![expected.to_owned()]),
                "{scope}"
            );
            assert!(!listed.items[0].now.exists, "{scope}");
        }
    }

    /// A proposal that does not exist is not found; a body that does not parse is malformed.
    #[tokio::test]
    async fn each_malformed_proposal_request_answers_its_problem() {
        let world = World::start().await;
        let ann = world.vendor_after(2).await;
        let step = json!({"patch_id": "p_step", "base_revision": 1});
        let nested = json!({
            "patch_id": "p_nested",
            "id": "pr_nested",
            "draft": {"title": "Nested", "destination_revision": 2, "mutations": [{"op": "discard_proposal"}]},
        });
        let cases = [
            (
                Method::GET,
                "/api/journeys/j_vendor_eval/proposals/pr_missing",
                None,
                ProblemCode::NotFound,
            ),
            (
                Method::POST,
                "/api/journeys/j_vendor_eval/proposals/pr_missing/discard",
                Some(&step),
                ProblemCode::NotFound,
            ),
            (
                Method::POST,
                "/api/journeys/j_vendor_eval/proposals/pr_missing/refresh",
                Some(&step),
                ProblemCode::NotFound,
            ),
            (
                Method::POST,
                "/api/journeys/j_vendor_eval/proposals/pr_missing/preview",
                None,
                ProblemCode::NotFound,
            ),
            (
                Method::PATCH,
                "/api/journeys/j_vendor_eval/proposals/pr_missing",
                Some(&step),
                ProblemCode::BadRequest,
            ),
            (
                Method::POST,
                "/api/journeys/j_vendor_eval/proposals",
                Some(&nested),
                ProblemCode::BadRequest,
            ),
        ];
        for (method, target, body, code) in cases {
            let reply = ann.send(method.clone(), target, body).await.unwrap();
            let problem: Problem = reply.json().unwrap();
            assert_eq!(
                (reply.status, problem.error),
                (cairn_api::error::status_of(code), code),
                "{method} {target}"
            );
        }
    }
    /// E6, I6: a deployment proposal that merges several identities into one and one that
    /// deletes an identity are read, previewed with the engine's candidate deployment, and
    /// applied.
    #[tokio::test]
    async fn identity_merges_and_deletes_are_proposed_previewed_and_applied() {
        let world = World::start().await;
        world.vendor_after(2).await;
        let ann = world.signed_in("ann");
        let deployment: cairn_schema::Deployment = get(&ann, "/api/deployment").await;
        let spare = "who_u5k3mq7wd2fz6ncx";
        let create = json!({"patch": {"id": "p_spare", "target": "deployment",
            "base_revision": deployment.revision.get(),
            "mutations": [{"op": "create_identity", "identity": {"key": spare, "name": "Spare"}}]}});
        ok::<Value>(&post(&ann, "/api/deployment/patches", &create).await);
        let revision = deployment.revision.get() + 1;
        let merge = |journeys: Value| {
            json!({"op": "merge_identities", "survivor": "who_d4m6uk2eqf5rbhzs",
                "merged": ["who_e7h2jv3nxc4yaw6t", spare], "journeys": journeys})
        };
        // A merge that names none of the journeys referring to its identities is previewed as
        // the violation it is; one that names them is clean, as is a delete.
        let proposals = [
            ("pr_unnamed", merge(json!({})), 1),
            ("pr_merge", merge(json!({"j_vendor_eval": 2})), 0),
            (
                "pr_delete",
                json!({"op": "delete_identity", "identity": spare}),
                0,
            ),
        ];
        for (id, mutation, violations) in proposals {
            let body = json!({"patch_id": format!("p_{id}"), "id": id, "draft": {
                "title": id, "destination_revision": revision, "mutations": [mutation]}});
            ok::<ProposalAnswer>(&post(&ann, "/api/deployment/proposals", &body).await);
            let held: Proposal = get(&ann, &format!("/api/deployment/proposals/{id}")).await;
            assert_eq!(held.status, ProposalStatus::Open);
            let review: ProposalReview = ok(&post(
                &ann,
                &format!("/api/deployment/proposals/{id}/preview"),
                &json!({}),
            )
            .await);
            assert_eq!(review.preview.violations.len(), violations, "{id}");
        }
    }
}
