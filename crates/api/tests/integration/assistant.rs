//! The assistant's endpoints in process (PRACTICES, Shell: API and MCP tests; I5, A12):
//! mounted only by a root that assembled the assistant, which the capabilities document
//! says; a turn answers its writes as the OpenAPI document describes, runs past the API's
//! request duration under its own limits, and is refused to an agent token.
#![cfg(test)]

mod in_process {
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::time::Duration;

    use axum::body::Body;
    use axum::extract::ConnectInfo;
    use axum::http::{Request, StatusCode};
    use cairn_api::client::Reply;
    use cairn_api::endpoints::{self as at, Endpoint};
    use cairn_api::wire::{Capabilities, MintedToken, Problem, ProblemCode, TurnReply};
    use cairn_assistant::scripted::{ScriptedProvider, Step};
    use cairn_assistant::{Action, Ended, Provider, Reply as ModelReply};
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use cairn_schema::Mutation;

    use cairn_api::client::Transport;

    use crate::support::{World, get, ok, post, scenario};

    const JOURNEY: &str = "j_vendor_eval";

    fn scripted(steps: Vec<Step>) -> Arc<ScriptedProvider> {
        Arc::new(ScriptedProvider::new(steps))
    }

    async fn with_assistant(provider: &Arc<ScriptedProvider>) -> World {
        let shared: Arc<dyn Provider> = Arc::clone(provider) as Arc<dyn Provider>;
        World::start_with_assistant(shared).await
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
        let capabilities: Capabilities = get(&ann, "/capabilities").await;
        assert!(!capabilities.assistant);
        let said = json!({ "message": "Hello." });
        for target in [
            "/journeys/j_vendor_eval/assistant",
            "/routes/vendor-evaluation/draft/assistant",
        ] {
            let reply = post(&ann, target, &said).await;
            assert_eq!(reply.status, StatusCode::NOT_FOUND, "{target}");
        }

        let provider = scripted(vec![Step::say("Hello."), Step::say("Hello again.")]);
        let with = with_assistant(&provider).await;
        let ann = with.vendor_after(1).await;
        let capabilities: Capabilities = get(&ann, "/capabilities").await;
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
    }

    /// I5, D7: a turn's direct write and its proposal are answered as the document says, the
    /// write with its consequences, the proposal by id.
    #[tokio::test]
    async fn a_turn_answers_its_writes_as_documented() {
        let reach = json!({ "journey": JOURNEY, "node": "n_kickoff", "transition": "reach",
            "patch_id": "p_kickoff", "base_revision": 2 });
        let add = json!({ "op": "add_node", "node": { "key": "n_extra", "id": "extra",
            "kind": "action", "title": "An extra step" } });
        let structure = json!({ "patch": { "id": "p_extra", "target": { "journey": JOURNEY },
            "base_revision": 3, "mutations": [add] } });
        let provider = scripted(vec![
            Step::call("transition_node", reach),
            Step::call("apply_patch", structure),
            Step::say("Kickoff is reached; the extra step is proposed."),
        ]);
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
        assert!(matches!(
            turn.actions.as_slice(),
            [Action::Applied { .. }, Action::Proposed { .. }]
        ));
        assert_eq!(
            world.revision(JOURNEY).await,
            3,
            "only the state change landed"
        );
    }

    /// I7, H2: the assistant acts for a signed-in user; an agent token is refused, and a
    /// journey that does not exist is not found.
    #[tokio::test]
    async fn an_agent_token_is_refused_and_a_missing_journey_not_found() {
        let provider = scripted(Vec::new());
        let world = with_assistant(&provider).await;
        let ann = world.vendor_after(1).await;
        let minted = post(&ann, "/users/me/tokens", &json!({ "name": "Helper" })).await;
        let minted: MintedToken = ok(&minted);
        let helper = world.bearer(&minted.token);
        let said = json!({ "message": "Hello." });
        let target = at::ASSISTANT_JOURNEY.path_with(&[JOURNEY]);
        let refused = post(&helper, &target, &said).await;
        assert_eq!(refused.status, StatusCode::FORBIDDEN);
        assert_eq!(
            refused.json::<Problem>().unwrap().error,
            ProblemCode::UserOnly
        );
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
        assert_eq!(provider.exchanges().len(), 0, "no provider call was made");
    }

    /// PRACTICES, Explicit limits: a turn is not held to the API's five-second request
    /// duration; a provider call past its own limit ends the turn, reported.
    #[tokio::test(start_paused = true)]
    async fn a_turn_runs_past_the_request_duration_under_its_own_limits() {
        let slow = ModelReply {
            text: Some("Done, slowly.".to_owned()),
            ..ModelReply::default()
        };
        let provider = scripted(vec![Step::Slow(Duration::from_secs(30), slow), Step::Hang]);
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
        for (message, ended) in [
            ("Slowly.", Ended::Replied),
            ("Again.", Ended::ProviderTimedOut),
        ] {
            let response = turn(message).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{message}");
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let answered: TurnReply = serde_json::from_slice(&body).unwrap();
            assert_eq!(answered.ended, ended, "{message}");
        }
    }

    /// The mutations a scenario step's proposal drafts, as JSON.
    fn proposed(fixture: &str, step: usize) -> Value {
        let scenario = scenario(fixture);
        let patch = &scenario.steps.as_slice()[step].patch;
        let Mutation::CreateProposal { proposal } = &patch.mutations.as_slice()[0] else {
            panic!("step {step} drafts no proposal");
        };
        serde_json::to_value(proposal.mutations.as_slice()).unwrap()
    }

    /// I5, I7, H2, D7, PRD Success criteria: a conversation over HTTP. An empty
    /// journey with no route is structured through one proposal the user applies; a state
    /// change applies directly with its consequences; a request touching more than ten
    /// nodes becomes one proposal; and a provider that stops answering is reported.
    #[tokio::test(start_paused = true)]
    async fn a_conversation_structures_reports_proposes_and_times_out() {
        let provider = scripted(Vec::new());
        let world = with_assistant(&provider).await;
        let ann = world.vendor_after(1).await;
        let bakeoff = scenario("bake-off");
        let empty = json!({ "patch": bakeoff.steps.as_slice()[0].patch });
        ok::<Value>(&post(&ann, "/journeys/j_bakeoff/patches", &empty).await);

        structure_an_ad_hoc_journey(&world, &provider, &ann).await;
        answer_directly(&world, &provider, &ann).await;
        reweigh_in_bulk(&world, &provider, &ann).await;
        time_out(&world, &provider, &ann).await;
    }

    /// The empty bake-off journey structured through one proposal the user applies (H2, I7).
    async fn structure_an_ad_hoc_journey(
        world: &World,
        provider: &ScriptedProvider,
        ann: &Transport,
    ) {
        let structure = json!({ "patch": { "id": "p_bakeoff_structure",
            "target": { "journey": "j_bakeoff" }, "base_revision": 1,
            "mutations": proposed("bake-off", 1) },
            "note": "Structure a two-week bake-off between two options" });
        let read = json!({ "journey": "j_bakeoff" });
        provider.push([
            Step::call("get_snapshot", read.clone()),
            Step::call("apply_patch", structure),
            Step::say("I drafted the bake-off's structure as one proposal for you to review."),
        ]);
        let said = json!({ "message": "A two-week bake-off between two options: three \
            decisions, four deliverables, and a decision milestone. Set it up." });
        let reply = post(ann, "/journeys/j_bakeoff/assistant", &said).await;
        let turn: TurnReply = ok(&reply);
        let [Action::Proposed { proposal, .. }] = turn.actions.as_slice() else {
            panic!("{:?}", turn.actions);
        };
        assert_eq!(
            world.revision("j_bakeoff").await,
            1,
            "drafting moves nothing"
        );
        let apply = json!({ "patch_id": "p_apply_bakeoff", "reviewed_revision": 1 });
        let applied = post(ann, &format!("/proposals/{proposal}/apply"), &apply).await;
        ok::<Value>(&applied);
        assert_eq!(world.revision("j_bakeoff").await, 2);
        let events: Value = get(ann, "/events?patch=p_apply_bakeoff&size=1").await;
        let event = &events["items"][0]["event"];
        assert_eq!(event["actor"]["agent"], "ag_assistant");
        assert_eq!(event["confirming_user"], event["actor"]["user"]);
    }

    /// The up-front decisions answered directly, reported with what they caused (D7).
    async fn answer_directly(world: &World, provider: &ScriptedProvider, ann: &Transport) {
        // The bake-off's entities moved the deployment to revision 2.
        let mut answers = proposed_answers();
        for answer in answers.as_array_mut().unwrap() {
            if answer["decision"] == "n_meeting_date" {
                answer["value"] = json!({ "date": "2026-10-06" });
            }
        }
        let answer = json!({ "patch": { "id": "p_up_front", "target": { "journey": JOURNEY },
            "base_revision": 1, "deployment_revision": 2, "mutations": answers } });
        provider.push([
            Step::call("apply_patch", answer),
            Step::say("Answered. The decision meeting on the 6th leaves the work before it short."),
        ]);
        let said = json!({ "message": "Answer the up-front decisions: purchase, I own it, \
            both stakeholders informed, no partner, and the meeting is on the 6th." });
        let reply = post(ann, "/journeys/j_vendor_eval/assistant", &said).await;
        let turn: TurnReply = ok(&reply);
        assert!(
            matches!(turn.actions.as_slice(), [Action::Applied { consequences, .. }]
            if consequences.values().any(|caused| !caused.shortfalls.is_empty()))
        );
        assert_eq!(world.revision(JOURNEY).await, 2, "the answers landed");
    }

    /// Eleven weight overrides drafted as one proposal, nothing landing (I5).
    async fn reweigh_in_bulk(world: &World, provider: &ScriptedProvider, ann: &Transport) {
        let nodes = [
            "n_access",
            "n_plan",
            "n_plan_draft",
            "n_plan_review",
            "n_workload",
            "n_baseline",
            "n_criteria",
            "n_partner_results",
            "n_findings",
            "n_final_report",
            "n_decision_meeting",
        ];
        let weights: Vec<Value> = nodes
            .iter()
            .map(|node| json!({ "op": "set_node_field", "node": node, "value": { "weight": 2 } }))
            .collect();
        let reweigh = json!({ "patch": { "id": "p_reweigh", "target": { "journey": JOURNEY },
            "base_revision": 2, "mutations": weights } });
        provider.push([
            Step::call("apply_patch", reweigh),
            Step::say("That touches eleven nodes, so it is a proposal for you to review."),
        ]);
        let said = json!({ "message": "Lower the weight of every work item to 2." });
        let reply = post(ann, "/journeys/j_vendor_eval/assistant", &said).await;
        let turn: TurnReply = ok(&reply);
        assert!(matches!(turn.actions.as_slice(), [Action::Proposed { .. }]));
        assert_eq!(world.revision(JOURNEY).await, 2, "nothing landed");
    }

    /// A provider that stops answering, after a write that stands.
    async fn time_out(world: &World, provider: &ScriptedProvider, ann: &Transport) {
        let snooze = json!({ "journey": JOURNEY, "node": "n_access", "until": { "date":
            "2026-10-05" }, "patch_id": "p_snooze_access", "base_revision": 2 });
        provider.push([Step::call("snooze", snooze), Step::Hang]);
        let said = json!({ "message": "Snooze the access request until Monday, then tell me \
            what is next." });
        let reply = post(ann, "/journeys/j_vendor_eval/assistant", &said).await;
        let turn: TurnReply = ok(&reply);
        assert_eq!(turn.ended, Ended::ProviderTimedOut);
        assert!(matches!(turn.actions.as_slice(), [Action::Applied { .. }]));
        assert_eq!(
            world.revision(JOURNEY).await,
            3,
            "the write before it stands"
        );
    }

    /// The vendor evaluation's up-front answers (its scenario's second step), as JSON.
    fn proposed_answers() -> Value {
        let scenario = scenario("vendor-evaluation");
        serde_json::to_value(scenario.steps.as_slice()[1].patch.mutations.as_slice()).unwrap()
    }
}
