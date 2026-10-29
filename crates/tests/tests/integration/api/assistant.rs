//! The assistant's endpoints in process (PRACTICES, Shell: API and MCP tests; I5, A12):
//! mounted only by a root that assembled the assistant, which the capabilities document
//! says; a turn answers its writes as the OpenAPI document describes, runs past the API's
//! request duration under its own limits, and is refused to an agent token.
#![cfg(test)]

mod in_process {
    use std::net::SocketAddr;
    use std::time::Duration;

    use axum::body::Body;
    use axum::extract::ConnectInfo;
    use axum::http::Method;
    use axum::http::{Request, StatusCode};
    use cairn_api::client::Reply;
    use cairn_api::endpoints::{self as at, Endpoint};
    use cairn_api::wire::{Conversation, ConversationAuthor, MintedToken, TurnReply};
    use cairn_assistant::Ended;
    use cairn_service::wire::{Capabilities, Problem, ProblemCode};
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use cairn_schema::Domain;

    use crate::api::support::{World, get, ok, post};
    use crate::assistant::provider::{Provider, Step};

    const JOURNEY: &str = "j_vendor_eval";

    async fn scripted(steps: Vec<Step>) -> Provider {
        let provider = Provider::start().await;
        provider.script(steps);
        provider
    }

    async fn with_assistant(provider: &Provider) -> World {
        World::start_with_assistant(provider).await
    }

    /// Where `reply` departs from the document's schema for `endpoint` at its status.
    fn violations(endpoint: &Endpoint, reply: &Reply) -> Vec<String> {
        let document = cairn_api::openapi::document();
        let method = endpoint.method.as_str().to_ascii_lowercase();
        let operation = &document["paths"][endpoint.path][&method];
        let content = &operation["responses"][reply.status.as_str()]["content"];
        let schema = &content["application/json"]["schema"];
        assert!(
            schema.is_object(),
            "{} answered {}, undocumented",
            endpoint.operation,
            reply.status
        );
        let root = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "allOf": [schema],
            "components": document["components"],
        });
        let validator = jsonschema::draft202012::new(&root).unwrap();
        let body: Value = serde_json::from_slice(&reply.body).unwrap();
        validator
            .iter_errors(&body)
            .map(|error| error.to_string())
            .collect()
    }

    /// I5 (fully usable without the assistant): a root without it says so in its
    /// capabilities and serves neither endpoint; a root with it says so and serves both.
    #[tokio::test]
    async fn the_capabilities_say_whether_the_assistant_is_served() {
        let without = World::start().await;
        let ann = without.vendor_after(1).await;
        let capabilities: Capabilities = get(&ann, "/api/capabilities").await;
        assert!(!capabilities.assistant);
        let said = json!({ "message": "Hello." });
        for target in [
            "/api/journeys/j_vendor_eval/assistant",
            "/api/routes/vendor-evaluation/draft/assistant",
        ] {
            let reply = post(&ann, target, &said).await;
            assert_eq!(reply.status, StatusCode::NOT_FOUND, "{target}");
            let read = ann.send(Method::GET, target, None).await.unwrap();
            assert_eq!(read.status, StatusCode::NOT_FOUND, "{target}");
        }

        let provider = scripted(vec![Step::say("Hello."), Step::say("Hello again.")]).await;
        let with = with_assistant(&provider).await;
        let ann = with.vendor_after(1).await;
        let capabilities: Capabilities = get(&ann, "/api/capabilities").await;
        assert!(capabilities.assistant);
        let endpoints = [
            (&at::ASSISTANT_JOURNEY, JOURNEY),
            (&at::ASSISTANT_ROUTE_DRAFT, "vendor-evaluation"),
        ];
        for (endpoint, id) in endpoints {
            let reply = post(&ann, &endpoint.path_with(&[id]), &said).await;
            assert_eq!(reply.status, StatusCode::OK, "{}", endpoint.operation);
            assert_eq!(violations(endpoint, &reply), Vec::<String>::new());
        }
        let reads = [
            (&at::ASSISTANT_JOURNEY_CONVERSATION, JOURNEY),
            (&at::ASSISTANT_ROUTE_DRAFT_CONVERSATION, "vendor-evaluation"),
        ];
        for (endpoint, id) in reads {
            let reply = ann
                .send(Method::GET, &endpoint.path_with(&[id]), None)
                .await
                .unwrap();
            assert_eq!(reply.status, StatusCode::OK, "{}", endpoint.operation);
            assert_eq!(violations(endpoint, &reply), Vec::<String>::new());
        }
    }

    /// I5: a conversation is the caller's, one per target: read back as kept, the user's
    /// words, Cairn's report of each write, and the reply in order, with the same id the
    /// turn answered; another user's conversation about the same journey is their own.
    #[tokio::test]
    async fn a_conversation_reads_back_as_kept_and_only_by_its_user() {
        let complete = json!({ "journey": JOURNEY, "patch_id": "p_kickoff", "base_revision": 2,
            "mutations": [{ "op": "transition", "node": "n_kickoff", "transition": "complete" }] });
        let provider = scripted(vec![
            Step::call("update_journey_state", complete),
            Step::say("Kickoff is done."),
        ])
        .await;
        let world = with_assistant(&provider).await;
        let ann = world.vendor_after(2).await;
        let target = at::ASSISTANT_JOURNEY_CONVERSATION.path_with(&[JOURNEY]);
        let before: Conversation = get(&ann, &target).await;
        assert_eq!(before.messages, Vec::new(), "nothing kept before a turn");

        let said = json!({ "message": "Kickoff happened." });
        let turn: TurnReply = ok(&post(&ann, &target, &said).await);
        let kept: Conversation = get(&ann, &target).await;
        assert_eq!(kept.conversation, turn.conversation);
        assert_eq!(kept.conversation, before.conversation);
        let authors: Vec<ConversationAuthor> =
            kept.messages.iter().map(|message| message.author).collect();
        assert_eq!(
            authors,
            [
                ConversationAuthor::User,
                ConversationAuthor::Cairn,
                ConversationAuthor::Assistant
            ]
        );

        let bob: Conversation = get(&world.signed_in("bob"), &target).await;
        assert_ne!(bob.conversation, kept.conversation);
        assert_eq!(bob.messages, Vec::new());
    }

    /// I5, D7: a turn's direct write and its proposal are answered as the document says, the
    /// write with its consequences, the proposal by id.
    #[tokio::test]
    async fn a_turn_answers_its_writes_as_documented() {
        let complete = json!({ "journey": JOURNEY, "patch_id": "p_kickoff", "base_revision": 2,
            "mutations": [{ "op": "transition", "node": "n_kickoff", "transition": "complete" }] });
        let add = json!({ "op": "add_node", "node": { "key": "n_extra", "id": "extra",
            "kind": "action", "title": "An extra step" } });
        let create = json!({ "proposal": "pr_extra", "destination": { "journey": JOURNEY },
            "draft": { "title": "Add a step", "destination_revision": 3, "mutations": [add] },
            "patch_id": "p_extra" });
        let provider = scripted(vec![
            Step::call("update_journey_state", complete),
            Step::call("create_proposal", create),
            Step::say("Kickoff is done; the extra step is proposed."),
        ])
        .await;
        let world = with_assistant(&provider).await;
        let ann = world.vendor_after(2).await;
        let said = json!({ "message": "Kickoff happened; add an extra step." });
        let reply = post(&ann, &at::ASSISTANT_JOURNEY.path_with(&[JOURNEY]), &said).await;
        assert_eq!(
            violations(&at::ASSISTANT_JOURNEY, &reply),
            Vec::<String>::new()
        );
        let turn: TurnReply = ok(&reply);
        assert_eq!(turn.ended, Ended::Replied);
        let [write] = turn.writes.as_slice() else {
            panic!("{:?}", turn.writes);
        };
        assert_eq!(write.revision.to_string(), "3");
        let [proposed] = turn.proposals.as_slice() else {
            panic!("{:?}", turn.proposals);
        };
        assert_eq!(
            proposed.destination,
            Domain::Journey(JOURNEY.parse().unwrap())
        );
        assert_eq!(
            world.revision(JOURNEY).await,
            3,
            "only the state change landed"
        );
    }

    /// H2, I5: every endpoint for users only refuses an agent token before its handler runs,
    /// whatever it was asked; one that is not, such as listing the user's tokens, answers it.
    #[tokio::test]
    async fn every_endpoint_for_users_only_refuses_an_agent() {
        let provider = scripted(Vec::new()).await;
        let world = with_assistant(&provider).await;
        let ann = world.vendor_after(1).await;
        let minted = post(&ann, "/api/users/me/tokens", &json!({ "name": "Helper" })).await;
        let minted: MintedToken = ok(&minted);
        let helper = world.bearer(&minted.token);
        let for_users: Vec<&Endpoint> = at::ALL.into_iter().filter(|e| e.user_only).collect();
        assert_ne!(for_users, [] as [&cairn_api::endpoints::Endpoint; 0]);
        for endpoint in for_users {
            let segments = vec!["x"; endpoint.path.matches('{').count()];
            let path = endpoint.path_with(&segments);
            let body = (endpoint.method != Method::GET).then(|| json!({}));
            let reply = helper
                .send(endpoint.method.clone(), &path, body.as_ref())
                .await
                .unwrap();
            assert_eq!(
                reply.status,
                StatusCode::FORBIDDEN,
                "{}",
                endpoint.operation
            );
            let problem = reply.json::<Problem>().unwrap();
            assert_eq!(
                problem.error,
                ProblemCode::UserOnly,
                "{}",
                endpoint.operation
            );
        }
        let listed = helper
            .send(Method::GET, at::TOKENS.path, None)
            .await
            .unwrap();
        assert_eq!(listed.status, StatusCode::OK);
    }

    /// I5: a turn about a journey that does not exist is not found, as the document says,
    /// and no provider call is made.
    #[tokio::test]
    async fn a_turn_about_a_missing_journey_is_not_found() {
        let provider = scripted(Vec::new()).await;
        let world = with_assistant(&provider).await;
        let ann = world.vendor_after(1).await;
        let said = json!({ "message": "Hello." });
        let missing = post(
            &ann,
            &at::ASSISTANT_JOURNEY.path_with(&["j_nowhere"]),
            &said,
        )
        .await;
        assert_eq!(missing.status, StatusCode::NOT_FOUND);
        assert_eq!(
            violations(&at::ASSISTANT_JOURNEY, &missing),
            Vec::<String>::new()
        );
        assert_eq!(provider.request_count(), 0, "no provider call was made");
    }

    /// PRACTICES, Explicit limits: a turn is not held to the API's five-second request
    /// duration; a provider call past its own limit ends the turn, reported.
    #[tokio::test(start_paused = true)]
    async fn a_turn_runs_past_the_request_duration_under_its_own_limits() {
        let provider = scripted(vec![
            Step::say("Done, slowly.").after(Duration::from_secs(30)),
            Step::hang(),
        ])
        .await;
        let world = with_assistant(&provider).await;
        world.vendor_after(1).await;
        let turn = |message: &str| {
            let body = json!({ "message": message }).to_string();
            let request = Request::post(at::ASSISTANT_JOURNEY.path_with(&[JOURNEY]))
                .header("authorization", "Bearer team-ann")
                .header("content-type", "application/json")
                .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40_000))))
                .body(Body::from(body))
                .unwrap();
            world.router.clone().oneshot(request)
        };
        let mut stopped = Vec::new();
        for message in ["Slowly.", "Again."] {
            let response = turn(message).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{message}");
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let answered: TurnReply = serde_json::from_slice(&body).unwrap();
            stopped.push(answered.ended);
        }
        assert_eq!(stopped[0], Ended::Replied);
        assert!(
            matches!(stopped[1], Ended::Stopped { .. }),
            "{:?}",
            stopped[1]
        );
    }

    /// A message sent while a reply in the same conversation is still being written is told
    /// to wait, with its own problem code, not the one for an overloaded process.
    #[tokio::test(start_paused = true)]
    async fn a_message_sent_during_a_reply_is_told_to_wait() {
        let provider = scripted(vec![Step::hang()]).await;
        let world = with_assistant(&provider).await;
        let ann = world.vendor_after(1).await;
        let said = json!({ "message": "Hello." });
        let target = at::ASSISTANT_JOURNEY.path_with(&[JOURNEY]);
        let (_, again) = tokio::join!(post(&ann, &target, &said), post(&ann, &target, &said));
        assert_eq!(again.status, StatusCode::CONFLICT);
        let problem = again.json::<Problem>().unwrap();
        assert_eq!(problem.error, ProblemCode::TurnInProgress);
    }
}
