//! The built-in OAuth server (I2): an MCP client discovers the metadata, registers, has a
//! user sign in through OIDC and allow it, and exchanges the code for an agent token (H2);
//! every check on the way refuses what it should.

#[cfg(test)]
mod oauth {
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::http::header::{LOCATION, WWW_AUTHENTICATE};
    use cairn_auth::{AuthProvider, OAuthConfig, OAuthServer, OidcConfig, OidcProvider};
    use cairn_schema::Actor;
    use cairn_store::{AuthStore, UserRecord};
    use openidconnect::PkceCodeChallenge;
    use serde_json::json;

    use crate::auth::support::flow::{
        REDIRECT, authorize_uri, client_id, exchange, get, json_of, local, location, post_form,
        query, register,
    };
    use crate::auth::support::issuer::{Issuer, Person, Spoil};
    use crate::auth::support::{
        LOCAL, World, app, base, body, cookie_header, cookies_set, request, send, whoami,
        with_cookie,
    };

    const ANN: Person = Person {
        subject: "ann",
        name: "Ann",
        email: "ann@example.org",
        email_verified: true,
    };

    struct Setup {
        world: World,
        router: Router,
        issuer: Issuer,
    }

    /// The built-in server, federating sign-in to an OIDC stub.
    async fn setup() -> Setup {
        let world = World::new();
        let issuer = Issuer::start(world.clock.clock()).await;
        let config = OidcConfig {
            name: "oidc".parse().unwrap(),
            issuer: issuer.url(),
            client_id: "cairn".to_owned(),
            client_secret: None,
        };
        let oidc = OidcProvider::new(config, world.accounts.clone(), &base()).unwrap();
        let config = OAuthConfig {
            name: "cairn".parse().unwrap(),
            sign_in_with: Some("oidc".parse().unwrap()),
        };
        let server = OAuthServer::new(config, world.accounts.clone(), &base());
        let providers: Vec<Arc<dyn AuthProvider>> = vec![Arc::new(oidc), Arc::new(server)];
        let router = app(&world.auth(providers));
        Setup {
            world,
            router,
            issuer,
        }
    }

    /// The step secret on a consent page.
    async fn step_of(response: axum::http::Response<Body>) -> String {
        let html = String::from_utf8(body(response).await).unwrap();
        let start = html.find("name=\"step\" value=\"").unwrap() + "name=\"step\" value=\"".len();
        html[start..].split('"').next().unwrap().to_owned()
    }

    /// A signed-in browser for a user, as its session cookie.
    async fn signed_in(world: &World, user: &str) -> String {
        let record = UserRecord {
            id: user.parse().unwrap(),
            created_at: world.accounts.clock().now(),
        };
        world.store.put_user(record).await.unwrap();
        let session = world
            .accounts
            .start_session(&user.parse().unwrap())
            .await
            .unwrap();
        format!("cairn_session={}", session.expose())
    }

    /// The user at `cookies` allows a fresh client: its client id, the PKCE verifier, and
    /// the code its redirect URI received.
    async fn allowed_code(setup: &Setup, cookies: &str) -> (String, String, String) {
        let client = client_id(&setup.router).await;
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let authorize = authorize_uri(&client, REDIRECT, challenge.as_str(), &[]);
        let shown = get(&setup.router, &authorize, cookies).await;
        assert_eq!(shown.status(), StatusCode::OK);
        let step = step_of(shown).await;
        let form = [("step", step.as_str()), ("decision", "allow")];
        let answered = post_form(&setup.router, "/api/oauth/authorize", cookies, &form).await;
        let back = query(&location(&answered));
        (client, verifier.secret().clone(), back["code"].clone())
    }

    /// Follows the server's redirect to the OIDC sign-in, approves at the stub, and returns
    /// the session cookie, checking the sign-in returns to the authorization.
    async fn sign_in_through_oidc(setup: &Setup, sign_in: &str, authorize: &str) -> String {
        let started = get(&setup.router, sign_in, "").await;
        let at_issuer = location(&started);
        let code = setup.issuer.approve(&at_issuer, &ANN, Spoil::Nothing);
        let state = query(&at_issuer)["state"].clone();
        let callback = format!("/api/auth/oidc/callback?code={code}&state={state}");
        let browser = cookie_header(&cookies_set(&started));
        let finished = get(&setup.router, &callback, &browser).await;
        assert_eq!(location(&finished), authorize);
        let set = cookies_set(&finished);
        let session = set.iter().find(|(name, _)| name == "cairn_session");
        format!("cairn_session={}", session.unwrap().1)
    }

    /// I2 end to end: from a 401 an MCP client finds the metadata, registers (asking for
    /// refresh too, as Claude Code does), is sent to sign in through OIDC and to allow it,
    /// and exchanges the code for a token that acts as an agent for the user.
    #[tokio::test]
    async fn an_mcp_client_discovers_registers_and_gets_an_agent_token() {
        let setup = setup().await;
        let router = &setup.router;
        let refused = get(router, "/whoami", "").await;
        assert_eq!(refused.status(), StatusCode::UNAUTHORIZED);
        let challenge = refused.headers()[WWW_AUTHENTICATE].to_str().unwrap();
        let resource = json_of(get(router, &local(challenge.split('"').nth(1).unwrap()), "").await);
        let resource = resource.await;
        assert_eq!(resource["resource"], "http://127.0.0.1:8080/whoami");
        let issuer = resource["authorization_servers"][0].as_str().unwrap();
        let server = json_of(get(router, "/.well-known/oauth-authorization-server", "").await);
        let server = server.await;
        assert_eq!(server["issuer"], issuer);
        assert_eq!(server["code_challenge_methods_supported"], json!(["S256"]));
        let registered = register(router, REDIRECT).await;
        assert_eq!(registered.status(), StatusCode::CREATED);
        let registered = json_of(registered).await;
        assert_eq!(registered["grant_types"], json!(["authorization_code"]));
        let client = registered["client_id"].as_str().unwrap();

        let (pkce, verifier) = PkceCodeChallenge::new_random_sha256();
        let authorize = authorize_uri(client, REDIRECT, pkce.as_str(), &[]);
        let to_sign_in = get(router, &authorize, "").await;
        assert_eq!(to_sign_in.status(), StatusCode::SEE_OTHER);
        let cookies = sign_in_through_oidc(&setup, &location(&to_sign_in), &authorize).await;
        let step = step_of(get(router, &authorize, &cookies).await).await;
        let form = [("step", step.as_str()), ("decision", "allow")];
        let answered = post_form(router, "/api/oauth/authorize", &cookies, &form).await;
        let back = query(&location(&answered));
        assert_eq!(
            (back["state"].as_str(), back["iss"].as_str()),
            ("client-state", issuer)
        );

        let token = exchange(router, client, verifier.secret(), &back["code"]).await;
        assert_eq!(token.status(), StatusCode::OK);
        let token = json_of(token).await;
        let bearer = token["access_token"].as_str().unwrap();
        let as_agent =
            request("/whoami", LOCAL).header("authorization", format!("Bearer {bearer}"));
        let agent = whoami(router, as_agent.body(Body::empty()).unwrap()).await;
        let agent = agent.unwrap();
        let session = with_cookie(request("/whoami", LOCAL), &cookies);
        let user = whoami(router, session.body(Body::empty()).unwrap()).await;
        assert_eq!(agent.user, user.unwrap().user);
        let tokens = setup.world.accounts.tokens_of(&agent.user).await.unwrap();
        assert_eq!(Some(&tokens[0].agent), agent.agent.as_ref());
        assert_eq!(tokens[0].name.as_str(), "Claude Code");
    }

    /// Only https or loopback redirect URIs register, and a request naming another URI than
    /// the registered one is answered with a page, never sent there; PKCE is required.
    #[tokio::test]
    async fn redirect_uris_and_pkce_are_checked() {
        let setup = setup().await;
        let router = &setup.router;
        for (uri, status) in [
            ("https://client.example/callback", StatusCode::CREATED),
            ("http://client.example/callback", StatusCode::BAD_REQUEST),
        ] {
            assert_eq!(register(router, uri).await.status(), status, "{uri}");
        }
        let cookies = signed_in(&setup.world, "u_ann").await;
        let client = client_id(router).await;
        let (challenge, _) = PkceCodeChallenge::new_random_sha256();
        let other = authorize_uri(
            &client,
            "http://127.0.0.1:33419/callback",
            challenge.as_str(),
            &[],
        );
        let response = get(router, &other, &cookies).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(!response.headers().contains_key(LOCATION));
        let foreign = [("resource", "https://elsewhere.example/api/mcp")];
        let uri = authorize_uri(&client, REDIRECT, challenge.as_str(), &foreign);
        let response = get(router, &uri, &cookies).await;
        assert_eq!(query(&location(&response))["error"], "invalid_target");
        let plain = authorize_uri(&client, REDIRECT, "", &[]).replace("S256", "plain");
        let response = get(router, &plain, &cookies).await;
        assert_eq!(query(&location(&response))["error"], "invalid_request");
    }

    /// The code is single use, short-lived, and bound to its client and PKCE verifier.
    #[tokio::test]
    async fn a_code_is_exchanged_once_by_its_client_with_its_verifier() {
        let setup = setup().await;
        let router = &setup.router;
        let cookies = signed_in(&setup.world, "u_ann").await;
        let (client, verifier, code) = allowed_code(&setup, &cookies).await;
        let (wrong, _) = PkceCodeChallenge::new_random_sha256();
        let refused = exchange(router, &client, wrong.as_str(), &code).await;
        assert_eq!(json_of(refused).await["error"], "invalid_grant");
        let replayed = exchange(router, &client, &verifier, &code).await;
        assert_eq!(
            json_of(replayed).await["error"],
            "invalid_grant",
            "a refused exchange uses the code"
        );

        let (_, verifier, code) = allowed_code(&setup, &cookies).await;
        let other = register(router, "https://other.example/callback").await;
        let other = json_of(other).await["client_id"]
            .as_str()
            .unwrap()
            .to_owned();
        let foreign = exchange(router, &other, &verifier, &code).await;
        assert_eq!(json_of(foreign).await["error"], "invalid_grant");

        let (client, verifier, code) = allowed_code(&setup, &cookies).await;
        setup.world.clock.advance(61);
        let late = exchange(router, &client, &verifier, &code).await;
        assert_eq!(json_of(late).await["error"], "invalid_grant");

        let (client, verifier, code) = allowed_code(&setup, &cookies).await;
        let first = exchange(router, &client, &verifier, &code).await;
        assert_eq!(first.status(), StatusCode::OK);
    }

    /// The consent is the shown user's own: another user's browser cannot answer it, an
    /// answer is used once, a denial reaches the client as `access_denied`, and an agent
    /// cannot authorize a client at all.
    #[tokio::test]
    async fn only_the_user_asked_answers_the_consent_and_agents_cannot() {
        let setup = setup().await;
        let router = &setup.router;
        let ann = signed_in(&setup.world, "u_ann").await;
        let bob = signed_in(&setup.world, "u_bob").await;
        let client = client_id(router).await;
        let (challenge, _) = PkceCodeChallenge::new_random_sha256();
        let authorize = authorize_uri(&client, REDIRECT, challenge.as_str(), &[]);
        let answer = |cookies: String, step: String, decision: &'static str| async move {
            let form = [("step", step.as_str()), ("decision", decision)];
            post_form(router, "/api/oauth/authorize", &cookies, &form).await
        };

        let step = step_of(get(router, &authorize, &ann).await).await;
        let as_bob = answer(bob, step.clone(), "allow").await;
        assert_eq!(as_bob.status(), StatusCode::FORBIDDEN);
        let again = answer(ann.clone(), step, "allow").await;
        assert_eq!(again.status(), StatusCode::BAD_REQUEST, "the step was used");

        let step = step_of(get(router, &authorize, &ann).await).await;
        let denied = answer(ann.clone(), step, "deny").await;
        assert_eq!(query(&location(&denied))["error"], "access_denied");

        let actor = Actor {
            user: "u_ann".parse().unwrap(),
            agent: None,
        };
        let minted = setup
            .world
            .accounts
            .mint_token(&actor, "bot".parse().unwrap());
        let minted = minted.await.unwrap();
        let bearer = format!("Bearer {}", minted.token.expose());
        let as_agent = request(&authorize, LOCAL).header("authorization", bearer);
        let response = send(router, as_agent.body(Body::empty()).unwrap()).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
