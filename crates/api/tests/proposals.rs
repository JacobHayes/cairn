//! Proposals over HTTP, in process against the memory store (PRACTICES, Shell: API tests):
//! created by an agent under its own id and fetched rather than duplicated when resubmitted,
//! edited against their editing revision without moving the journey, previewed, and applied
//! by a reviewer recorded as confirming them (I6, H2, H5, C14); an apply against a moved
//! journey is stale with what intervened until the proposal is refreshed (I6); each malformed
//! proposal request answers its problem.
#![cfg(test)]

mod support;

mod in_process {
    use axum::http::{Method, StatusCode};
    use cairn_api::client::{Reply, Transport};
    use cairn_api::wire::{
        EventPage, MintedToken, PatchAnswer, Problem, ProblemCode, ProposalAnswer, ProposalReview,
        Viewer,
    };
    use cairn_schema::{Proposal, ProposalStatus, Rejection, RevisionOf};
    use serde_json::{Value, json};

    use crate::support::{World, get, ok, post};

    const JOURNEY: &str = "/journeys/j_vendor_eval";

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
            ok(&post(&world.signed_in(member), "/users/me/tokens", &named).await);
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
        world.vendor_quietly(3).await;
        let agent = agent_of(world, "ann").await;
        let proposals = format!("{JOURNEY}/proposals");
        ok::<ProposalAnswer>(&post(&agent, &proposals, &create_body("p_pr")).await);
        let edited = patch(&agent, "/proposals/pr_note", &edit_body("p_edit", 1)).await;
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
        let held: Proposal = get(&agent, "/proposals/pr_note").await;
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
        let old = patch(&agent, "/proposals/pr_note", &edit_body("p_edit_old", 1)).await;
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
            ok(&post(&bob, "/proposals/pr_note/preview", &json!({})).await);
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
            "/proposals/pr_note/apply",
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
            ok(&post(&bob, "/proposals/pr_note/apply", &apply_body("p_apply", 2)).await);
        assert_eq!(applied.receipt().revision.get(), 4);
        let resubmitted = post(&bob, "/proposals/pr_note/apply", &apply_body("p_apply", 2)).await;
        let resubmitted: PatchAnswer = ok(&resubmitted);
        let receipt = applied.receipt().clone();
        assert_eq!(resubmitted, PatchAnswer::AlreadyApplied { receipt }, "H5");
        let held: Proposal = get(&bob, "/proposals/pr_note").await;
        assert_eq!(held.status, ProposalStatus::Applied);
        let reviewer: Viewer = get(&bob, "/users/me").await;
        let events: EventPage = get(&bob, "/events?patch=p_apply").await;
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
        let ann = world.vendor_quietly(3).await;
        for (_, target, id, draft) in crate::support::proposals_of_every_domain(&ann).await {
            let create = json!({"patch_id": format!("p_{id}"), "id": id, "draft": draft});
            let created = saved(ok(&post(&ann, &target, &create).await));
            let review: ProposalReview =
                ok(&post(&ann, &format!("/proposals/{id}/preview"), &json!({})).await);
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
                    "kickoff, reached, now waits on the charter"
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
        let ann = world.vendor_quietly(3).await;
        let create = create_body("p_pr");
        ok::<ProposalAnswer>(&post(&ann, &format!("{JOURNEY}/proposals"), &create).await);
        let discard = json!({"patch_id": "p_discard", "base_revision": 1});
        let discarded: ProposalAnswer =
            ok(&post(&ann, "/proposals/pr_note/discard", &discard).await);
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
        let ann = world.vendor_quietly(3).await;
        let create = create_body("p_pr");
        ok::<ProposalAnswer>(&post(&ann, &format!("{JOURNEY}/proposals"), &create).await);
        let other = crate::support::patch(
            "p_other",
            "{journey: j_vendor_eval}",
            3,
            "- op: add_annotation\n  annotation: {key: a_other, note: Meanwhile.}\n",
        );
        ok::<PatchAnswer>(
            &post(
                &ann,
                &format!("{JOURNEY}/patches"),
                &crate::support::request(&other),
            )
            .await,
        );

        let review: ProposalReview =
            ok(&post(&ann, "/proposals/pr_note/preview", &json!({})).await);
        let stale = review.stale.expect("the journey moved");
        assert_eq!(
            (stale.conflict.expected.get(), stale.conflict.current.get()),
            (3, 4)
        );
        assert!(!stale.intervening.is_empty());
        let Rejection::Stale {
            conflicts,
            intervening,
        } = rejection(
            &post(
                &ann,
                "/proposals/pr_note/apply",
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
            ok(&post(&ann, "/proposals/pr_note/refresh", &refresh).await);
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
            ok(&post(&ann, "/proposals/pr_note/preview", &json!({})).await);
        assert_eq!(review.stale, None);
        let applied: PatchAnswer =
            ok(&post(&ann, "/proposals/pr_note/apply", &apply_body("p_apply", 2)).await);
        assert_eq!(applied.receipt().revision.get(), 5);
    }

    /// A proposal that does not exist is not found; a body that does not parse is malformed.
    #[tokio::test]
    async fn each_malformed_proposal_request_answers_its_problem() {
        let world = World::start().await;
        let ann = world.vendor_quietly(2).await;
        let step = json!({"patch_id": "p_step", "base_revision": 1});
        let nested = json!({
            "patch_id": "p_nested",
            "id": "pr_nested",
            "draft": {"title": "Nested", "destination_revision": 2, "mutations": [{"op": "discard_proposal"}]},
        });
        let cases = [
            (
                Method::GET,
                "/proposals/pr_missing",
                None,
                ProblemCode::NotFound,
            ),
            (
                Method::POST,
                "/proposals/pr_missing/discard",
                Some(&step),
                ProblemCode::NotFound,
            ),
            (
                Method::POST,
                "/proposals/pr_missing/refresh",
                Some(&step),
                ProblemCode::NotFound,
            ),
            (
                Method::POST,
                "/proposals/pr_missing/preview",
                None,
                ProblemCode::NotFound,
            ),
            (
                Method::PATCH,
                "/proposals/pr_missing",
                Some(&step),
                ProblemCode::BadRequest,
            ),
            (
                Method::POST,
                "/journeys/j_vendor_eval/proposals",
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
}
