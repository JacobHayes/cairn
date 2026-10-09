//! The reads, the caller, and agent tokens over HTTP, in process against the memory store
//! (PRACTICES, Shell: API tests): each answers what the service or auth holds (C16, C17, I2,
//! E6, H2, H3, J5), the domain document derives as the server would, and each malformed
//! read is answered with its problem.
#![cfg(test)]

mod in_process {
    use axum::http::{Method, StatusCode};
    use cairn_api::error::status_of;
    use cairn_api::wire::{
        AgentToken, Capabilities, EventPage, JourneyPage, MintedToken, Problem, ProblemCode,
        RouteDetail, RoutePage, SearchHit, SearchPage, Viewer,
    };
    use cairn_engine::{Graph, derive};
    use cairn_schema::{Deployment, DomainDocument, Entity, Journey, Route, RouteVersion};

    use crate::support::{self, World, get, post};

    const JOURNEY: &str = "/api/journeys/j_vendor_eval";

    /// I2: the route index lists every route in id order, a page at a time, each with its
    /// latest version and whether a draft is open.
    #[tokio::test]
    async fn the_route_index_pages_in_id_order() {
        let world = World::start().await;
        let ann = world.vendor_after(0).await;
        let second = support::publish_fixture_route("hiring-loop");
        let reply = post(
            &ann,
            "/api/routes/hiring-loop/patches",
            &support::request(&second),
        )
        .await;
        support::ok::<serde_json::Value>(&reply);

        let first: RoutePage = get(&ann, "/api/routes?size=1").await;
        let after = first.next.clone().unwrap();
        let rest: RoutePage = get(&ann, &format!("/api/routes?after={after}")).await;
        assert_eq!(rest.next, None);
        let listed: Vec<(String, Option<u32>, bool)> = first
            .items
            .iter()
            .chain(&rest.items)
            .map(|route| {
                let latest = route.latest_version.map(cairn_schema::VersionNumber::get);
                (route.header.id.to_string(), latest, route.draft_open)
            })
            .collect();
        let published = |id: &str| (id.to_owned(), Some(1), false);
        assert_eq!(
            listed,
            [published("hiring-loop"), published("vendor-evaluation")]
        );
    }

    /// A21, C19: a segment is inserted through the service, the route index filters by kind,
    /// and the segment's detail lists where its version is inserted.
    #[tokio::test]
    async fn a_segment_is_listed_by_kind_and_lists_its_insertions() {
        let world = World::start().await;
        let ann = world.vendor_after(1).await;
        // Two process routes sort before the segment, so a page of one must look past them.
        for name in ["hiring-loop", "product-launch"] {
            let seed = support::publish_fixture_route(name);
            let path = format!("/api/routes/{name}/patches");
            support::ok::<serde_json::Value>(&post(&ann, &path, &support::request(&seed)).await);
        }
        let seed = support::publish_fixture_route("security-review");
        support::ok::<serde_json::Value>(
            &post(
                &ann,
                "/api/routes/security-review/patches",
                &support::request(&seed),
            )
            .await,
        );
        let insert = "- op: insert_segment\n  insertion: i_review\n  segment: {route: security-review, version: 1}\n";
        let patch = support::patch("p_insert", "{journey: j_vendor_eval}", 1, insert);
        let reply = post(
            &ann,
            "/api/journeys/j_vendor_eval/patches",
            &support::request(&patch),
        )
        .await;
        support::ok::<serde_json::Value>(&reply);

        let listed: RoutePage = get(&ann, "/api/routes?kind=segment&size=1").await;
        assert_eq!(listed.items.len(), 1);
        let detail: RouteDetail = get(&ann, "/api/routes/security-review/versions").await;
        assert_eq!(
            detail.versions[0].insertions[0].title.as_str(),
            "Security review"
        );
    }

    /// Every read answers what the service holds (C16, C17, E6, H3, J5), and capabilities
    /// what the host offers.
    #[tokio::test]
    async fn every_read_answers_what_the_service_holds() {
        let world = World::start().await;
        let ann = world.vendor_after(2).await;
        let service = &world.service;
        let id = "j_vendor_eval".parse().unwrap();
        let route_id = "vendor-evaluation".parse().unwrap();

        let journey: Journey = get(&ann, JOURNEY).await;
        assert_eq!(Some(journey), service.journey(&id).await.unwrap());
        let route: Route = get(&ann, "/api/routes/vendor-evaluation").await;
        assert_eq!(Some(route), service.route(&route_id).await.unwrap());
        let version: RouteVersion = get(&ann, "/api/routes/vendor-evaluation/versions/1").await;
        assert_eq!(version.version.get(), 1);
        let detail: RouteDetail = get(&ann, "/api/routes/vendor-evaluation/versions").await;
        assert_eq!(detail.versions[0].journeys, [id.clone()].into());
        let deployment: Deployment = get(&ann, "/api/deployment").await;
        assert_eq!(deployment, service.deployment().await.unwrap());
        let entity: Entity = get(&ann, "/api/entities/e_lead").await;
        assert_eq!(entity.key.as_str(), "e_lead");

        let index: JourneyPage = get(&ann, "/api/journeys?status=active&size=1").await;
        assert_eq!((index.items.len(), index.next), (1, None));
        assert!(!index.items[0].upgrade_available);
        let none: JourneyPage = get(&ann, "/api/journeys?status=archived").await;
        assert_eq!(none.items.len(), 0);
        let found: SearchPage = get(&ann, "/api/search?text=ANALYTICS").await;
        assert!(found.items[0].hits.contains(&SearchHit::JourneyName));
        let events: EventPage = get(
            &ann,
            "/api/events?log=journey:j_vendor_eval&type=answer_set&size=2",
        )
        .await;
        assert_eq!(events.items.len(), 2);
        assert!(events.next.is_some(), "five answers, two per page");
        let rest: EventPage = get(
            &ann,
            &format!(
                "/api/events?log=journey:j_vendor_eval&type=answer_set&after={}",
                events.next.unwrap()
            ),
        )
        .await;
        assert_eq!(rest.items.len(), 3);

        let viewer: Viewer = get(&ann, "/api/users/me").await;
        assert_eq!(viewer.entities, ["e_lead".parse().unwrap()].into());
        assert_eq!((viewer.agent, viewer.merge_offer), (None, None));
        let emails: Vec<_> = viewer
            .identities
            .iter()
            .flat_map(|identity| &identity.verified_emails)
            .collect();
        assert!(
            !viewer.identities.is_empty() && !emails.is_empty(),
            "the identity that names the entity is listed with its verified email"
        );
        let capabilities: Capabilities = get(&ann, "/api/capabilities").await;
        assert_eq!(capabilities, Capabilities::from(service.capabilities()));
        assert!(capabilities.sse && capabilities.mcp && !capabilities.assistant);
    }

    /// The domain document carries the stored journey and the caller's derive inputs and
    /// nothing derived; deriving it gives what the server derives at the same inputs.
    #[tokio::test]
    async fn a_document_derives_to_what_the_server_derives() {
        let world = World::start().await;
        let ann = world.vendor_after(3).await;
        world.clock.set("2026-10-06T03:00:00Z");
        let sent: serde_json::Value = get(&ann, &format!("{JOURNEY}/document")).await;
        let document: DomainDocument = serde_json::from_value(sent.clone()).unwrap();
        let stored = world
            .service
            .journey(&"j_vendor_eval".parse().unwrap())
            .await
            .unwrap();
        assert_eq!(Some(&document.journey), stored.as_ref());
        assert_eq!(
            sent["journey"],
            serde_json::to_value(stored.unwrap()).unwrap()
        );
        assert_eq!(
            document.inputs.today.to_string(),
            "2026-10-05",
            "today in the deployment's zone"
        );
        assert_eq!(document.inputs.viewer, ["e_lead".parse().unwrap()].into());

        let deployment = world.service.deployment().await.unwrap();
        let today = support::settings().today("2026-10-06T03:00:00Z".parse().unwrap());
        let inputs =
            support::settings().derive_inputs(today, document.inputs.viewer.clone(), deployment);
        assert_eq!(inputs, document.inputs);
        let journey = &document.journey;
        let graph = Graph::new(journey.graph.clone(), &inputs.deployment).unwrap();
        let on_server = derive(&graph, Some(journey.header.created_on), &inputs);
        let received =
            Graph::new(document.journey.graph.clone(), &document.inputs.deployment).unwrap();
        let in_browser = derive(
            &received,
            Some(document.journey.header.created_on),
            &document.inputs,
        );
        assert_eq!(in_browser, on_server);
    }

    /// H2: a user mints an agent token, which then signs requests in as an agent acting for
    /// them; an agent can neither mint nor revoke; a revoked token is refused.
    #[tokio::test]
    async fn an_agent_token_acts_for_its_user_until_revoked() {
        let world = World::start().await;
        let ann = world.signed_in("ann");
        let named = serde_json::json!({ "name": "nightly report" });
        let reply = post(&ann, "/api/users/me/tokens", &named).await;
        assert_eq!(reply.status, StatusCode::CREATED);
        let minted: MintedToken = reply.json().unwrap();
        let agent = world.bearer(&minted.token);
        let as_agent: Viewer = get(&agent, "/api/users/me").await;
        let as_user: Viewer = get(&ann, "/api/users/me").await;
        assert_eq!(
            (&as_agent.user, as_agent.agent.as_ref()),
            (&as_user.user, Some(&minted.agent))
        );

        let refused = post(&agent, "/api/users/me/tokens", &named).await;
        assert_eq!(refused.status, StatusCode::FORBIDDEN);
        assert_eq!(
            refused.json::<Problem>().unwrap().error,
            ProblemCode::UserOnly
        );
        let listed: Vec<AgentToken> = get(&ann, "/api/users/me/tokens").await;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].agent, minted.agent);

        let revoke = format!("/api/users/me/tokens/{}", minted.agent);
        let revoked = ann.send(Method::DELETE, &revoke, None).await.unwrap();
        assert_eq!(revoked.status, StatusCode::NO_CONTENT);
        let after = agent
            .send(Method::GET, "/api/users/me", None)
            .await
            .unwrap();
        assert_eq!(after.status, StatusCode::UNAUTHORIZED);
        let missing = ann
            .send(Method::DELETE, "/api/users/me/tokens/ag_nobody", None)
            .await
            .unwrap();
        assert_eq!(missing.status, StatusCode::NOT_FOUND);
    }

    /// Each malformed read is answered with its problem at its status.
    #[tokio::test]
    async fn a_malformed_read_is_answered_with_its_problem() {
        let world = World::start().await;
        let ann = world.vendor_after(1).await;
        let cases = [
            ("/api/journeys/not-a-journey-id", ProblemCode::BadRequest),
            ("/api/journeys/j_missing", ProblemCode::NotFound),
            ("/api/journeys/%FF", ProblemCode::BadRequest),
            (
                "/api/routes/vendor-evaluation/versions/two",
                ProblemCode::BadRequest,
            ),
            (
                "/api/routes/vendor-evaluation/versions/9",
                ProblemCode::NotFound,
            ),
            ("/api/entities/e_nobody", ProblemCode::NotFound),
            ("/api/journeys?colour=blue", ProblemCode::BadRequest),
            ("/api/journeys?route=r_a&route=r_b", ProblemCode::BadRequest),
            ("/api/events?log=node:n_a", ProblemCode::BadRequest),
            ("/api/search", ProblemCode::BadRequest),
        ];
        for (target, code) in cases {
            let reply = ann.send(Method::GET, target, None).await.unwrap();
            assert_eq!(reply.status, status_of(code), "{target}");
            assert_eq!(reply.json::<Problem>().unwrap().error, code, "{target}");
        }
        let wrong = ann.send(Method::DELETE, JOURNEY, None).await.unwrap();
        assert_eq!(wrong.status, StatusCode::METHOD_NOT_ALLOWED);
    }
}
