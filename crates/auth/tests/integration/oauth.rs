//! The built-in OAuth server (I2: MCP is an OAuth resource server on the shared auth): an
//! MCP client discovers the resource and server metadata, registers, has a user sign in
//! through OIDC and allow it, and exchanges the code for an agent token (H2); every check
//! on the way refuses what it should (3.2, Security).

#[cfg(test)]
mod oauth {
    use std::collections::HashMap;
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::header::{CONTENT_SECURITY_POLICY, CONTENT_TYPE, LOCATION, WWW_AUTHENTICATE};
    use axum::http::{Response, StatusCode};
    use cairn_auth::{AuthProvider, OAuthConfig, OAuthServer, OidcConfig, OidcProvider};
    use cairn_schema::{Actor, UserId};
    use cairn_store::{AuthStore, UserRecord};
    use openidconnect::PkceCodeChallenge;
    use serde_json::{Value, json};

    use crate::support::issuer::{Issuer, Person, Spoil};
    use crate::support::{
        LOCAL, World, app, base, body, cookie_header, cookies_set, loopback, request, send, whoami,
        with_cookie,
    };

    const REDIRECT: &str = "http://127.0.0.1:33418/callback";

    struct Setup {
        world: World,
        router: Router,
        issuer: Option<Issuer>,
    }

    /// The built-in server, federating sign-in to an OIDC stub when `federate` is set.
    async fn setup(federate: bool) -> Setup {
        let world = World::new();
        let mut providers: Vec<Arc<dyn AuthProvider>> = Vec::new();
        let issuer = if federate {
            Some(Issuer::start(world.clock.clock()).await)
        } else {
            None
        };
        if let Some(issuer) = &issuer {
            let config = OidcConfig {
                name: "oidc".parse().unwrap(),
                issuer: issuer.url(),
                client_id: "cairn".to_owned(),
                client_secret: None,
                auto_link: false,
            };
            providers.push(Arc::new(
                OidcProvider::new(config, world.accounts.clone(), &base()).unwrap(),
            ));
        }
        let config = OAuthConfig {
            name: "cairn".parse().unwrap(),
            sign_in_with: federate.then(|| "oidc".parse().unwrap()),
        };
        providers.push(Arc::new(OAuthServer::new(
            config,
            world.accounts.clone(),
            &base(),
        )));
        let router = app(&world.auth(loopback(), providers));
        Setup {
            world,
            router,
            issuer,
        }
    }

    async fn get(setup: &Setup, uri: &str, cookies: &str) -> Response<Body> {
        let request = with_cookie(request(uri, LOCAL), cookies).body(Body::empty());
        send(&setup.router, request.unwrap()).await
    }

    async fn post_form(
        setup: &Setup,
        uri: &str,
        cookies: &str,
        form: &[(&str, &str)],
    ) -> Response<Body> {
        let encoded = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(form)
            .finish();
        let request = with_cookie(request(uri, LOCAL), cookies)
            .method("POST")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(encoded));
        send(&setup.router, request.unwrap()).await
    }

    async fn json_of(response: Response<Body>) -> Value {
        serde_json::from_slice(&body(response).await).unwrap()
    }

    fn location(response: &Response<Body>) -> String {
        response.headers()[LOCATION].to_str().unwrap().to_owned()
    }

    fn query(location: &str) -> HashMap<String, String> {
        let url = url::Url::parse(location)
            .or_else(|_| url::Url::parse(&format!("http://cairn{location}")));
        url.unwrap().query_pairs().into_owned().collect()
    }

    /// The path and query of an absolute URL on this deployment.
    fn local(url: &str) -> String {
        let url = url::Url::parse(url).unwrap();
        format!(
            "{}{}",
            url.path(),
            url.query()
                .map(|query| format!("?{query}"))
                .unwrap_or_default()
        )
    }

    async fn register(setup: &Setup, redirect: &str) -> Response<Body> {
        let registration = json!({"redirect_uris": [redirect], "client_name": "Test client"});
        let request = request("/api/oauth/register", LOCAL)
            .method("POST")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(registration.to_string()));
        send(&setup.router, request.unwrap()).await
    }

    async fn client_id(setup: &Setup) -> String {
        let response = register(setup, REDIRECT).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        json_of(response).await["client_id"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn authorize_uri(client_id: &str, redirect: &str, challenge: &str) -> String {
        let pairs = [
            ("response_type", "code"),
            ("client_id", client_id),
            ("redirect_uri", redirect),
            ("state", "client-state"),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
        ];
        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(pairs)
            .finish();
        format!("/api/oauth/authorize?{query}")
    }

    /// The step secret on a consent page.
    async fn step_of(response: Response<Body>) -> String {
        let html = String::from_utf8(body(response).await).unwrap();
        let start = html.find("name=\"step\" value=\"").unwrap() + "name=\"step\" value=\"".len();
        html[start..].split('"').next().unwrap().to_owned()
    }

    async fn signed_in(world: &World, user: &str) -> String {
        let record = UserRecord {
            id: user.parse().unwrap(),
            name: "Someone".parse().unwrap(),
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
        let client = client_id(setup).await;
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let shown = get(
            setup,
            &authorize_uri(&client, REDIRECT, challenge.as_str()),
            cookies,
        )
        .await;
        assert_eq!(shown.status(), StatusCode::OK);
        let step = step_of(shown).await;
        let form = [("step", step.as_str()), ("decision", "allow")];
        let answered = post_form(setup, "/api/oauth/authorize", cookies, &form).await;
        let back = query(&location(&answered));
        (client, verifier.secret().clone(), back["code"].clone())
    }

    async fn exchange(setup: &Setup, client: &str, verifier: &str, code: &str) -> Response<Body> {
        let form = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", REDIRECT),
            ("client_id", client),
            ("code_verifier", verifier),
        ];
        post_form(setup, "/api/oauth/token", "", &form).await
    }

    /// I2 end to end: from a 401 an MCP client finds the resource and server metadata,
    /// registers, sends the user to authorize (who signs in through OIDC on the way and
    /// allows it), and exchanges the code for a token that acts as an agent for the user.
    #[tokio::test]
    async fn an_mcp_client_discovers_registers_and_gets_an_agent_token() {
        let setup = setup(true).await;
        let refused = get(&setup, "/whoami", "").await;
        assert_eq!(refused.status(), StatusCode::UNAUTHORIZED);
        let challenge = refused.headers()[WWW_AUTHENTICATE].to_str().unwrap();
        let metadata_url = challenge.split('"').nth(1).unwrap();
        let resource = json_of(get(&setup, &local(metadata_url), "").await).await;
        assert_eq!(resource["resource"], "http://127.0.0.1:8080/whoami");
        let issuer = resource["authorization_servers"][0].as_str().unwrap();
        let server =
            json_of(get(&setup, "/.well-known/oauth-authorization-server", "").await).await;
        assert_eq!(server["issuer"], issuer);
        assert_eq!(server["code_challenge_methods_supported"], json!(["S256"]));
        let register_path = local(server["registration_endpoint"].as_str().unwrap());
        assert_eq!(register_path, "/api/oauth/register");
        let client = client_id(&setup).await;

        let (pkce, verifier) = PkceCodeChallenge::new_random_sha256();
        let authorize = authorize_uri(&client, REDIRECT, pkce.as_str());
        let to_sign_in = get(&setup, &authorize, "").await;
        assert_eq!(to_sign_in.status(), StatusCode::SEE_OTHER);
        let cookies = sign_in_through_oidc(&setup, &location(&to_sign_in), &authorize).await;
        let shown = get(&setup, &authorize, &cookies).await;
        let policy = shown.headers()[CONTENT_SECURITY_POLICY].to_str().unwrap();
        let form_action = policy
            .split(';')
            .find(|directive| directive.trim().starts_with("form-action"));
        let sources: Vec<&str> = form_action.unwrap().split_whitespace().collect();
        assert!(
            sources.contains(&"http://127.0.0.1:33418"),
            "the answer may redirect: {policy}"
        );
        let step = step_of(shown).await;
        let form = [("step", step.as_str()), ("decision", "allow")];
        let answered = post_form(&setup, "/api/oauth/authorize", &cookies, &form).await;
        let back = location(&answered);
        assert!(back.starts_with(REDIRECT), "{back}");
        let back = query(&back);
        assert_eq!(
            (back["state"].as_str(), back["iss"].as_str()),
            ("client-state", issuer)
        );

        let token = exchange(&setup, &client, verifier.secret(), &back["code"]).await;
        assert_eq!(token.status(), StatusCode::OK);
        let token = json_of(token).await;
        assert_eq!(token["token_type"], "Bearer");
        let bearer = token["access_token"].as_str().unwrap();
        let as_agent =
            request("/whoami", LOCAL).header("authorization", format!("Bearer {bearer}"));
        let agent = whoami(&setup.router, as_agent.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let session = with_cookie(request("/whoami", LOCAL), &cookies)
            .body(Body::empty())
            .unwrap();
        let user = whoami(&setup.router, session).await.unwrap();
        assert_eq!(agent.user, user.user);
        let tokens = setup.world.accounts.tokens_of(&user.user).await.unwrap();
        assert_eq!(Some(&tokens[0].agent), agent.agent.as_ref());
        assert_eq!(tokens[0].name.as_str(), "Test client");
    }

    /// Follows the server's redirect to the OIDC sign-in, approves at the stub, and
    /// returns the session cookie, checking the sign-in returns to the authorization.
    async fn sign_in_through_oidc(setup: &Setup, sign_in: &str, authorize: &str) -> String {
        let started = get(setup, sign_in, "").await;
        let at_issuer = location(&started);
        let issuer = setup.issuer.as_ref().unwrap();
        let person = Person {
            subject: "ann",
            name: "Ann",
            email: "ann@example.org",
            email_verified: true,
        };
        let code = issuer.approve(&at_issuer, &person, Spoil::Nothing);
        let state = query(&at_issuer)["state"].clone();
        let callback = format!("/api/auth/oidc/callback?code={code}&state={state}");
        let finished = get(setup, &callback, &cookie_header(&cookies_set(&started))).await;
        assert_eq!(location(&finished), authorize);
        let set = cookies_set(&finished);
        let session = set
            .iter()
            .find(|(name, _)| name == "cairn_session")
            .unwrap();
        format!("cairn_session={}", session.1)
    }

    /// Redirect URIs match exactly, and only https or loopback ones register; a request
    /// naming another URI is answered with a page, never sent there.
    #[tokio::test]
    async fn redirect_uris_register_only_https_or_loopback_and_match_exactly() {
        let setup = setup(false).await;
        for (uri, status) in [
            ("https://client.example/callback", StatusCode::CREATED),
            ("http://client.example/callback", StatusCode::BAD_REQUEST),
        ] {
            assert_eq!(register(&setup, uri).await.status(), status, "{uri}");
        }
        let cookies = signed_in(&setup.world, "u_ann").await;
        let client = client_id(&setup).await;
        let (challenge, _) = PkceCodeChallenge::new_random_sha256();
        for other in [
            "http://127.0.0.1:33419/callback",
            "https://attacker.example/callback",
        ] {
            let response = get(
                &setup,
                &authorize_uri(&client, other, challenge.as_str()),
                &cookies,
            )
            .await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{other}");
            assert!(!response.headers().contains_key(LOCATION));
        }
        let missing_pkce =
            authorize_uri(&client, REDIRECT, "").replace("&code_challenge_method=S256", "");
        let response = get(&setup, &missing_pkce, &cookies).await;
        assert_eq!(query(&location(&response))["error"], "invalid_request");
    }

    /// The code is single use, short-lived, and bound to its client, redirect URI, and
    /// PKCE verifier.
    #[tokio::test]
    async fn a_code_is_exchanged_once_by_its_client_with_its_verifier() {
        let setup = setup(false).await;
        let cookies = signed_in(&setup.world, "u_ann").await;
        let (client, verifier, code) = allowed_code(&setup, &cookies).await;
        let (wrong, _) = PkceCodeChallenge::new_random_sha256();
        let refused = exchange(&setup, &client, wrong.as_str(), &code).await;
        assert_eq!(json_of(refused).await["error"], "invalid_grant");
        let replayed = exchange(&setup, &client, &verifier, &code).await;
        assert_eq!(
            json_of(replayed).await["error"],
            "invalid_grant",
            "a refused exchange uses the code"
        );

        let (client, verifier, code) = allowed_code(&setup, &cookies).await;
        let other = register(&setup, "https://other.example/callback").await;
        let other = json_of(other).await["client_id"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_ne!(other, client);
        let foreign = exchange(&setup, &other, &verifier, &code).await;
        assert_eq!(json_of(foreign).await["error"], "invalid_grant");

        let (client, verifier, code) = allowed_code(&setup, &cookies).await;
        setup.world.clock.advance(61);
        let late = exchange(&setup, &client, &verifier, &code).await;
        assert_eq!(json_of(late).await["error"], "invalid_grant");

        let (client, verifier, code) = allowed_code(&setup, &cookies).await;
        assert_eq!(
            exchange(&setup, &client, &verifier, &code).await.status(),
            StatusCode::OK
        );
        assert_eq!(
            exchange(&setup, &client, &verifier, &code).await.status(),
            StatusCode::BAD_REQUEST
        );
    }

    /// The consent is the shown user's own: another user's browser cannot answer it, an
    /// answer is used once, a denial reaches the client as `access_denied`, and an agent
    /// cannot authorize a client at all.
    #[tokio::test]
    async fn only_the_user_asked_answers_the_consent_and_agents_cannot() {
        let setup = setup(false).await;
        let ann = signed_in(&setup.world, "u_ann").await;
        let bob = signed_in(&setup.world, "u_bob").await;
        let client = client_id(&setup).await;
        let (challenge, _) = PkceCodeChallenge::new_random_sha256();
        let authorize = authorize_uri(&client, REDIRECT, challenge.as_str());

        let step = step_of(get(&setup, &authorize, &ann).await).await;
        let as_bob = post_form(
            &setup,
            "/api/oauth/authorize",
            &bob,
            &[("step", &step), ("decision", "allow")],
        )
        .await;
        assert_eq!(as_bob.status(), StatusCode::FORBIDDEN);
        let again = post_form(
            &setup,
            "/api/oauth/authorize",
            &ann,
            &[("step", &step), ("decision", "allow")],
        )
        .await;
        assert_eq!(again.status(), StatusCode::BAD_REQUEST, "the step was used");

        let step = step_of(get(&setup, &authorize, &ann).await).await;
        let denied = post_form(
            &setup,
            "/api/oauth/authorize",
            &ann,
            &[("step", &step), ("decision", "deny")],
        )
        .await;
        assert_eq!(query(&location(&denied))["error"], "access_denied");

        let user: UserId = "u_ann".parse().unwrap();
        let actor = Actor { user, agent: None };
        let minted = setup
            .world
            .accounts
            .mint_token(&actor, "bot".parse().unwrap())
            .await
            .unwrap();
        let as_agent = request(&authorize, LOCAL)
            .header("authorization", format!("Bearer {}", minted.token.expose()));
        let response = send(&setup.router, as_agent.body(Body::empty()).unwrap()).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let anonymous = get(&setup, &authorize, "").await;
        assert_eq!(
            anonymous.status(),
            StatusCode::UNAUTHORIZED,
            "no provider to sign in with"
        );
    }
    /// A lapsed session cookie is no dead end: the authorization clears it and sends the
    /// browser to sign in again.
    #[tokio::test]
    async fn a_lapsed_session_signs_in_again_rather_than_failing() {
        let setup = setup(true).await;
        let cookies = signed_in(&setup.world, "u_ann").await;
        let lifetime = cairn_auth::lifetimes::SESSION_LIFETIME.as_secs();
        setup.world.clock.advance(i64::try_from(lifetime).unwrap());
        let client = client_id(&setup).await;
        let (challenge, _) = PkceCodeChallenge::new_random_sha256();
        let response = get(
            &setup,
            &authorize_uri(&client, REDIRECT, challenge.as_str()),
            &cookies,
        )
        .await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert!(location(&response).starts_with("/api/auth/oidc/sign-in?"));
        assert_eq!(
            cookies_set(&response),
            [("cairn_session".to_owned(), String::new())]
        );
    }
    /// Every challenge names metadata the server serves, the deployment's root included.
    #[tokio::test]
    async fn every_challenge_names_served_resource_metadata() {
        let setup = setup(false).await;
        let config = OAuthConfig {
            name: "cairn".parse().unwrap(),
            sign_in_with: None,
        };
        let server = OAuthServer::new(config, setup.world.accounts.clone(), &base());
        for (path, resource) in [
            ("/", "http://127.0.0.1:8080"),
            ("/api/mcp", "http://127.0.0.1:8080/api/mcp"),
        ] {
            let challenge = server.challenge(path).unwrap();
            let metadata = local(challenge.split('"').nth(1).unwrap());
            let response = get(&setup, &metadata, "").await;
            assert_eq!(response.status(), StatusCode::OK, "{path}");
            assert_eq!(json_of(response).await["resource"], resource);
        }
    }

    /// A browser whose session lapsed but which another provider signs in (the dev user)
    /// goes on to the consent as that user, and the lapsed cookie is cleared.
    #[tokio::test]
    async fn a_lapsed_session_falls_back_to_the_provider_that_signs_the_browser_in() {
        let world = World::new();
        let dev = cairn_auth::DevConfig {
            name: "dev".parse().unwrap(),
            user: "Dev".parse().unwrap(),
            verified_emails: std::collections::BTreeSet::new(),
            token: None,
            auto_link: false,
            allow_off_loopback: false,
        };
        let config = OAuthConfig {
            name: "cairn".parse().unwrap(),
            sign_in_with: None,
        };
        let providers: Vec<Arc<dyn AuthProvider>> = vec![
            Arc::new(cairn_auth::DevProvider::new(dev, loopback()).unwrap()),
            Arc::new(OAuthServer::new(config, world.accounts.clone(), &base())),
        ];
        let router = app(&world.auth(loopback(), providers));
        let setup = Setup {
            world,
            router,
            issuer: None,
        };
        let lapsed = format!(
            "cairn_session={}",
            cairn_auth::Secret::mint(cairn_auth::secret::SecretKind::Session)
                .unwrap()
                .expose()
        );
        let client = client_id(&setup).await;
        let (challenge, _) = PkceCodeChallenge::new_random_sha256();
        let shown = get(
            &setup,
            &authorize_uri(&client, REDIRECT, challenge.as_str()),
            &lapsed,
        )
        .await;
        assert_eq!(shown.status(), StatusCode::OK);
        assert_eq!(
            cookies_set(&shown),
            [("cairn_session".to_owned(), String::new())]
        );
    }
}
