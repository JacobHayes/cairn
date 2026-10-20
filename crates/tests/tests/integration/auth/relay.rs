//! The IAP provider's OAuth relay (I2), offline against a stub of Google: registration,
//! the round trip through Google, and the token endpoint, each refusing what it should.
//! Cairn stays the issuer the client hears, stores nothing, keeps the Google secret to
//! itself, checks `resource` itself, and answers only with an ID token.

#[cfg(test)]
mod relay {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    use axum::Router;
    use axum::body::Body;
    use axum::http::header::{CONTENT_TYPE, LOCATION, SET_COOKIE};
    use axum::http::{Response, StatusCode};
    use base64::Engine as _;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use cairn_auth::limits::PROVIDER_CALL_DURATION_MAX;
    use cairn_auth::{AuthProvider, GcpIapConfig, GcpIapProvider, GoogleEndpoints, RelayClient};
    use cairn_store::AuthStore;
    use openidconnect::PkceCodeChallenge;
    use serde_json::{Value, json};
    use url::Url;

    use crate::auth::support::iap::AUDIENCE;
    use crate::auth::support::issuer::{Issuer, Person, Switches};
    use crate::auth::support::{
        REMOTE, World, app, base, body, cookie_header, cookies_set, public, request, send,
        with_cookie,
    };

    const GOOGLE_CLIENT: &str = "relay-client.apps.example";
    const GOOGLE_SECRET: &str = "relay-secret-value";
    const REDIRECT: &str = "http://localhost:59411/callback";
    const ISSUER: &str = "http://127.0.0.1:8080";

    const ANN: Person = Person {
        subject: "1001",
        name: "Ann",
        email: "ann@example.org",
        email_verified: true,
    };

    struct Setup {
        world: World,
        google: Issuer,
        router: Router,
    }

    fn relay_config() -> GcpIapConfig {
        GcpIapConfig {
            name: "iap".parse().unwrap(),
            audience: AUDIENCE.to_owned(),
            auto_link: false,
            relay: Some(RelayClient {
                client_id: GOOGLE_CLIENT.to_owned(),
                client_secret: GOOGLE_SECRET.to_owned(),
            }),
        }
    }

    /// The relay calling the Google stub, or `token` in its place.
    async fn setup_with_token(token: Option<Url>) -> Setup {
        let world = World::new();
        let google = Issuer::start_google(world.clock.clock(), GOOGLE_CLIENT, GOOGLE_SECRET).await;
        let endpoints = GoogleEndpoints {
            authorization: google.authorization_url(),
            token: token.unwrap_or_else(|| google.token_url()),
            ..GoogleEndpoints::google()
        };
        let clock = world.clock.clock();
        let iap = GcpIapProvider::with_endpoints(relay_config(), clock, &base(), endpoints);
        let iap: Arc<dyn AuthProvider> = Arc::new(iap.unwrap());
        let router = app(&world.auth(public(), vec![iap]));
        Setup {
            world,
            google,
            router,
        }
    }

    async fn setup() -> Setup {
        setup_with_token(None).await
    }

    async fn get(setup: &Setup, uri: &str, cookies: &str) -> Response<Body> {
        let request = with_cookie(request(uri, REMOTE), cookies).body(Body::empty());
        send(&setup.router, request.unwrap()).await
    }

    async fn post_form(setup: &Setup, form: &[(&str, &str)]) -> Response<Body> {
        let encoded = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(form)
            .finish();
        let request = request("/api/oauth/token", REMOTE)
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
        Url::parse(location)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect()
    }

    /// The pairs of a URL's query, repeats kept.
    fn pairs(location: &str) -> Vec<(String, String)> {
        Url::parse(location)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect()
    }

    /// The path and query of an absolute URL on this deployment.
    fn local(url: &str) -> String {
        let url = Url::parse(url).unwrap();
        assert!(url.as_str().starts_with(ISSUER), "{url}");
        format!("{}?{}", url.path(), url.query().unwrap_or_default())
    }

    /// What a local MCP client sends to register (Claude Code's body).
    fn claude_code_registration(redirect: &str) -> Value {
        json!({
            "client_name": "Claude Code (cairn)",
            "redirect_uris": [redirect],
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none",
        })
    }

    async fn register(setup: &Setup, registration: &Value) -> Response<Body> {
        let request = request("/api/oauth/register", REMOTE)
            .method("POST")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(registration.to_string()));
        send(&setup.router, request.unwrap()).await
    }

    async fn client_id(setup: &Setup) -> String {
        let registered = register(setup, &claude_code_registration(REDIRECT)).await;
        assert_eq!(registered.status(), StatusCode::CREATED);
        json_of(registered).await["client_id"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn authorize_uri(
        client: &str,
        redirect: &str,
        challenge: &str,
        extra: &[(&str, &str)],
    ) -> String {
        let mut pairs = vec![
            ("response_type", "code"),
            ("client_id", client),
            ("redirect_uri", redirect),
            ("state", "client-state"),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
        ];
        pairs.extend_from_slice(extra);
        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(pairs)
            .finish();
        format!("/api/oauth/authorize?{query}")
    }

    /// A sign-in begun: the authorization's answer (to Google, with the attempt's cookie).
    struct Begun {
        client: String,
        verifier: String,
        at_google: String,
        cookies: String,
    }

    async fn begin(setup: &Setup, extra: &[(&str, &str)]) -> Begun {
        let client = client_id(setup).await;
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let uri = authorize_uri(&client, REDIRECT, challenge.as_str(), extra);
        let to_google = get(setup, &uri, "").await;
        assert_eq!(to_google.status(), StatusCode::SEE_OTHER);
        Begun {
            client,
            verifier: verifier.secret().clone(),
            at_google: location(&to_google),
            cookies: cookie_header(&cookies_set(&to_google)),
        }
    }

    /// Ann signs in at Google, and the browser brings Google's answer to Cairn: Cairn's
    /// answer.
    async fn come_back(setup: &Setup, begun: &Begun) -> Response<Body> {
        let from_google = setup.google.sign_in(&begun.at_google, &ANN);
        get(setup, &local(&from_google), &begun.cookies).await
    }

    /// A whole sign-in: the client, its verifier, and the code it received.
    async fn signed_in(setup: &Setup) -> (String, String, String) {
        let begun = begin(setup, &[]).await;
        let back = come_back(setup, &begun).await;
        let code = query(&location(&back))["code"].clone();
        (begun.client, begun.verifier, code)
    }

    async fn exchange(setup: &Setup, client: &str, verifier: &str, code: &str) -> Response<Body> {
        let form = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", REDIRECT),
            ("client_id", client),
            ("code_verifier", verifier),
        ];
        post_form(setup, &form).await
    }

    /// Nothing durable: no user, login, or token (each would be in the auth log), and no
    /// session or OAuth state, however long from now.
    async fn stored_nothing(world: &World) -> bool {
        let later = world.clock.clock().now() + jiff::SignedDuration::from_hours(24 * 3650);
        let state = world.store.remove_expired(later).await.unwrap();
        world.log().await.is_empty() && state == 0
    }

    /// I2: registration is open, takes a local client's own body, answers with no secret
    /// and with both grants (so the client knows it may refresh), refuses any redirect but
    /// loopback, and stores nothing.
    #[tokio::test]
    async fn registration_is_open_loopback_only_and_stores_nothing() {
        let setup = setup().await;
        let registered = register(&setup, &claude_code_registration(REDIRECT)).await;
        assert_eq!(registered.status(), StatusCode::CREATED);
        let registered = json_of(registered).await;
        assert!(registered.get("client_secret").is_none(), "{registered}");
        assert_eq!(
            registered["grant_types"],
            json!(["authorization_code", "refresh_token"])
        );
        let web = claude_code_registration("https://client.example/callback");
        let refused = register(&setup, &web).await;
        assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
        assert_eq!(json_of(refused).await["error"], "invalid_redirect_uri");
        assert!(stored_nothing(&setup.world).await);
    }

    /// RFC 9207: the client hears Cairn as the issuer its metadata names, once, never
    /// Google, whose answer carried its own; on a refusal at Google too.
    #[tokio::test]
    async fn the_client_hears_cairn_as_issuer_never_google() {
        let setup = setup().await;
        let metadata = json_of(get(&setup, "/.well-known/oauth-authorization-server", "").await);
        let issuer = metadata.await["issuer"].as_str().unwrap().to_owned();
        assert_eq!(issuer, ISSUER);
        let begun = begin(&setup, &[]).await;
        let back = location(&come_back(&setup, &begun).await);
        assert!(back.starts_with(REDIRECT), "{back}");
        let issuers: Vec<_> = pairs(&back)
            .into_iter()
            .filter(|(k, _)| k == "iss")
            .collect();
        assert_eq!(issuers, [("iss".to_owned(), issuer.clone())]);
        assert_eq!(query(&back)["state"], "client-state");

        let begun = begin(&setup, &[]).await;
        let declined = setup.google.decline(&begun.at_google);
        let back = location(&get(&setup, &local(&declined), &begun.cookies).await);
        let back = pairs(&back);
        let issuers: Vec<_> = back.iter().filter(|(k, _)| k == "iss").collect();
        assert_eq!(issuers, [&("iss".to_owned(), issuer)]);
        assert!(back.contains(&("error".to_owned(), "access_denied".to_owned())));
        assert!(stored_nothing(&setup.world).await);
    }

    /// Google's `state` as Cairn sent it, with `change` made to the client request it
    /// carries.
    fn tampered(state: &str, change: impl Fn(&mut Value)) -> String {
        let mut carried: Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(state).unwrap()).unwrap();
        change(&mut carried);
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&carried).unwrap())
    }

    /// Google's answer for `begun`, with its state replaced by `state`.
    fn answer_with_state(setup: &Setup, begun: &Begun, state: &str) -> String {
        let from_google = setup.google.sign_in(&begun.at_google, &ANN);
        let mut url = Url::parse(&from_google).unwrap();
        let kept: Vec<(String, String)> = url
            .query_pairs()
            .into_owned()
            .filter(|(name, _)| name != "state")
            .collect();
        url.query_pairs_mut()
            .clear()
            .extend_pairs(kept)
            .append_pair("state", state);
        local(url.as_str())
    }

    /// The round trip through Google cannot be altered to send a code anywhere else: a
    /// redirect the client did not register, a state changed on the way, a callback in a
    /// browser that did not start the attempt, a state naming a URI no relay client may
    /// register even with a cookie to match, and an older attempt's answer once a newer
    /// one began, are each refused with a page and no redirect.
    #[tokio::test]
    async fn a_tampered_round_trip_sends_no_code() {
        let setup = setup().await;
        let client = client_id(&setup).await;
        let (challenge, _) = PkceCodeChallenge::new_random_sha256();
        let other = "http://localhost:59411/other";
        let uri = authorize_uri(&client, other, challenge.as_str(), &[]);
        let unregistered = get(&setup, &uri, "").await;
        assert_eq!(unregistered.status(), StatusCode::BAD_REQUEST);
        assert!(!unregistered.headers().contains_key(LOCATION));

        let begun = begin(&setup, &[]).await;
        let state = query(&begun.at_google)["state"].clone();
        let moved = tampered(&state, |carried| {
            carried["redirect_uri"] = json!("http://localhost:1/callback");
        });
        let elsewhere = tampered(&state, |carried| {
            carried["redirect_uri"] = json!("https://elsewhere.example/cb");
        });
        let elsewhere_cookie = format!(
            "cairn_relay_iap={}",
            cairn_auth::secret::digest(&elsewhere).as_str()
        );
        let cases = [
            (
                "state changed",
                answer_with_state(&setup, &begun, &moved),
                begun.cookies.clone(),
            ),
            (
                "another browser",
                answer_with_state(&setup, &begun, &state),
                String::new(),
            ),
            (
                "foreign redirect",
                answer_with_state(&setup, &begun, &elsewhere),
                elsewhere_cookie,
            ),
        ];
        for (case, callback, cookies) in cases {
            let refused = get(&setup, &callback, &cookies).await;
            assert_eq!(refused.status(), StatusCode::BAD_REQUEST, "{case}");
            assert!(!refused.headers().contains_key(LOCATION), "{case}");
        }

        let first = begin(&setup, &[]).await;
        let second = begin(&setup, &[]).await;
        let older = setup.google.sign_in(&first.at_google, &ANN);
        let refused = get(&setup, &local(&older), &second.cookies).await;
        assert_eq!(
            refused.status(),
            StatusCode::BAD_REQUEST,
            "an older attempt"
        );
        assert!(!refused.headers().contains_key(LOCATION));
        let kept = !refused.headers().contains_key(SET_COOKIE);
        assert!(kept, "the newer attempt's cookie is kept");
        let newer = come_back(&setup, &second).await;
        assert!(location(&newer).starts_with(REDIRECT));
        let cleared = newer.headers().get_all(SET_COOKIE).iter().count();
        assert_eq!(cleared, 1, "the attempt's cookie is cleared");
    }

    /// No ID token, no success: the token endpoint answers with Google's ID token or fails,
    /// never with Google's access token. Google's refusals of a code or refresh token are
    /// passed on as `invalid_grant`.
    #[tokio::test]
    async fn the_token_endpoint_answers_an_id_token_or_fails() {
        let setup = setup().await;
        let (client, verifier, code) = signed_in(&setup).await;
        let answered = json_of(exchange(&setup, &client, &verifier, &code).await).await;
        let refresh = answered["refresh_token"].as_str().unwrap().to_owned();
        assert!(
            setup
                .google
                .verified(answered["access_token"].as_str().unwrap(), GOOGLE_CLIENT)
                .is_some()
        );

        let (client, verifier, code) = signed_in(&setup).await;
        setup.google.switch(Switches {
            omit_id_token: true,
            hang: false,
        });
        let refresh_form = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh.as_str()),
            ("client_id", client.as_str()),
        ];
        let without = [
            ("code", exchange(&setup, &client, &verifier, &code).await),
            ("refresh", post_form(&setup, &refresh_form).await),
        ];
        for (case, response) in without {
            assert_eq!(response.status(), StatusCode::BAD_GATEWAY, "{case}");
            let text = String::from_utf8(body(response).await).unwrap();
            assert!(
                !text.contains("stub-access") && !text.contains("access_token"),
                "{case}: {text}"
            );
        }
        setup.google.switch(Switches::default());

        let (client, verifier, code) = signed_in(&setup).await;
        let (wrong, _) = PkceCodeChallenge::new_random_sha256();
        let wrong_verifier = format!("{}{}", wrong.as_str(), "x");
        let unknown = [
            ("grant_type", "refresh_token"),
            ("refresh_token", "stub-refresh-nope"),
            ("client_id", client.as_str()),
        ];
        let refused = [
            (
                "wrong verifier",
                exchange(&setup, &client, &wrong_verifier, &code).await,
            ),
            (
                "replayed code",
                exchange(&setup, &client, &verifier, &code).await,
            ),
            ("unknown refresh token", post_form(&setup, &unknown).await),
        ];
        for (case, response) in refused {
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{case}");
            let answer = json_of(response).await;
            assert_eq!(answer["error"], "invalid_grant", "{case}");
            assert!(answer.get("access_token").is_none(), "{case}");
        }
    }

    /// A redirect URI the client did not register is refused before Google is asked.
    #[tokio::test]
    async fn a_redirect_uri_the_client_did_not_register_is_refused_before_google() {
        let setup = setup().await;
        let (client, verifier, code) = signed_in(&setup).await;
        let calls = setup.google.token_forms().len();
        let foreign = [
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", "http://localhost:59411/other"),
            ("client_id", client.as_str()),
            ("code_verifier", verifier.as_str()),
        ];
        let refused = json_of(post_form(&setup, &foreign).await).await;
        assert_eq!(refused["error"], "invalid_grant");
        assert_eq!(
            setup.google.token_forms().len(),
            calls,
            "Google was not asked"
        );
    }

    /// A Google that cannot be reached fails the exchange as a server error.
    #[tokio::test]
    async fn an_unreachable_google_is_a_server_error() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let closed: Url = format!("http://{}/token", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        drop(listener);
        let setup = setup_with_token(Some(closed)).await;
        let (client, verifier, code) = signed_in(&setup).await;
        let response = exchange(&setup, &client, &verifier, &code).await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(json_of(response).await["error"], "server_error");
    }

    /// RFC 8707: `resource` is checked against this deployment and never sent to Google:
    /// a foreign one is refused at authorize (at the client, as Cairn) and at the token
    /// endpoint (before Google is asked); the deployment's own passes and goes no further.
    #[tokio::test]
    async fn resource_is_checked_here_and_never_sent_to_google() {
        let setup = setup().await;
        let client = client_id(&setup).await;
        let (challenge, _) = PkceCodeChallenge::new_random_sha256();
        let foreign = [("resource", "https://elsewhere.example/api/mcp")];
        let uri = authorize_uri(&client, REDIRECT, challenge.as_str(), &foreign);
        let back = query(&location(&get(&setup, &uri, "").await));
        assert_eq!(
            (back["error"].as_str(), back["iss"].as_str()),
            ("invalid_target", ISSUER)
        );

        let own = format!("{ISSUER}/api/mcp");
        let begun = begin(&setup, &[("resource", &own)]).await;
        let code = query(&location(&come_back(&setup, &begun).await))["code"].clone();
        let mut form = vec![
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", REDIRECT),
            ("client_id", begun.client.as_str()),
            ("code_verifier", begun.verifier.as_str()),
            ("resource", "https://elsewhere.example/api/mcp"),
        ];
        let refused = post_form(&setup, &form).await;
        assert_eq!(json_of(refused).await["error"], "invalid_target");
        assert!(
            setup.google.token_forms().is_empty(),
            "Google was not asked"
        );
        form.pop();
        form.push(("resource", &own));
        assert_eq!(post_form(&setup, &form).await.status(), StatusCode::OK);
        let at_google = setup.google.authorizations();
        let sent = setup.google.token_forms();
        assert!(!at_google[0].contains_key("resource") && !sent[0].contains_key("resource"));
    }

    /// The Google client secret goes only to Google's token endpoint: it is in no answer
    /// Cairn gives, and Google is sent Cairn's callback as the redirect URI, though the
    /// client sent its own.
    #[tokio::test]
    async fn the_google_secret_goes_only_to_googles_token_endpoint() {
        let setup = setup().await;
        let registered = register(&setup, &claude_code_registration(REDIRECT)).await;
        let mut seen = vec![String::from_utf8(body(registered).await).unwrap()];
        let begun = begin(&setup, &[]).await;
        seen.push(begun.at_google.clone());
        let back = come_back(&setup, &begun).await;
        let back = location(&back);
        seen.push(back.clone());
        let code = query(&back)["code"].clone();
        let answered = exchange(&setup, &begun.client, &begun.verifier, &code).await;
        seen.push(String::from_utf8(body(answered).await).unwrap());
        let replayed = exchange(&setup, &begun.client, &begun.verifier, &code).await;
        seen.push(String::from_utf8(body(replayed).await).unwrap());
        for answer in &seen {
            assert!(!answer.contains(GOOGLE_SECRET), "{answer}");
        }
        let sent = &setup.google.token_forms()[0];
        assert_eq!(sent["client_secret"], GOOGLE_SECRET);
        assert_eq!(sent["redirect_uri"], format!("{ISSUER}/api/oauth/callback"));
    }

    /// A request body that never arrives.
    struct Stalled;

    impl futures_core::Stream for Stalled {
        type Item = Result<axum::body::Bytes, std::convert::Infallible>;

        fn poll_next(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Self::Item>> {
            std::task::Poll::Pending
        }
    }

    /// A token request whose body trickles in forever ends at the deadline, with a server
    /// error and no token.
    #[tokio::test(start_paused = true)]
    async fn a_stalled_relay_request_ends_at_the_deadline() {
        let setup = setup().await;
        let request = request("/api/oauth/token", REMOTE)
            .method("POST")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header("content-length", "100")
            .body(Body::from_stream(Stalled))
            .unwrap();
        let started = tokio::time::Instant::now();
        let answered =
            tokio::time::timeout(PROVIDER_CALL_DURATION_MAX * 2, send(&setup.router, request));
        let answered = answered.await.unwrap();
        assert!(started.elapsed() <= PROVIDER_CALL_DURATION_MAX);
        assert!(answered.status().is_server_error(), "{}", answered.status());
        assert!(json_of(answered).await.get("access_token").is_none());
    }

    /// A Google that never answers ends the request at the deadline, with a server error
    /// and no token.
    #[tokio::test]
    async fn a_hanging_google_ends_at_the_deadline() {
        let setup = setup().await;
        let client = client_id(&setup).await;
        setup.google.switch(Switches {
            omit_id_token: false,
            hang: true,
        });
        let verifier = "v".repeat(43);
        let form = [
            ("grant_type", "authorization_code"),
            ("code", "a-code"),
            ("redirect_uri", REDIRECT),
            ("client_id", client.as_str()),
            ("code_verifier", verifier.as_str()),
        ];
        let encoded = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(form)
            .finish();
        let request = request("/api/oauth/token", REMOTE)
            .method("POST")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(encoded))
            .unwrap();
        let router = setup.router.clone();
        let answered = tokio::spawn(async move { send(&router, request).await });
        tokio::time::timeout(Duration::from_secs(5), setup.google.entered())
            .await
            .unwrap();
        tokio::time::pause();
        let paused = tokio::time::Instant::now();
        tokio::time::advance(PROVIDER_CALL_DURATION_MAX).await;
        let answered = tokio::time::timeout(PROVIDER_CALL_DURATION_MAX * 2, answered);
        let answered = answered.await.unwrap().unwrap();
        // A timer fires on the millisecond tick after its deadline, so within a second of it.
        let tick = Duration::from_secs(1);
        assert!(
            paused.elapsed() < PROVIDER_CALL_DURATION_MAX + tick,
            "{:?}",
            paused.elapsed()
        );
        assert!(answered.status().is_server_error(), "{}", answered.status());
        assert!(json_of(answered).await.get("access_token").is_none());
        assert_eq!(setup.google.token_forms().len(), 1);
    }
}
