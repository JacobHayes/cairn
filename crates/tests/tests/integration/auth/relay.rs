//! The IAP provider's OAuth relay (I2), offline against a stub of Google: registration, the
//! round trip through Google, and the token endpoint, each refusing what it should. Cairn
//! stays the issuer the client hears, stores nothing, keeps the Google secret to itself,
//! checks `resource` itself, and answers only with an ID token.

#[cfg(test)]
mod relay {
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

    use crate::auth::support::flow::{
        REDIRECT, authorize_uri, client_id, exchange, get, json_of, local, location, post_form,
        query, register,
    };
    use crate::auth::support::iap::AUDIENCE;
    use crate::auth::support::issuer::{Issuer, Person};
    use crate::auth::support::{
        LOCAL, World, app, base, body, cookie_header, cookies_set, request, send,
    };

    const GOOGLE_CLIENT: &str = "relay-client.apps.example";
    const GOOGLE_SECRET: &str = "relay-secret-value";
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

    async fn setup() -> Setup {
        let world = World::new();
        let google = Issuer::start_google(world.clock.clock(), GOOGLE_CLIENT, GOOGLE_SECRET).await;
        let endpoints = GoogleEndpoints {
            authorization: google.authorization_url(),
            token: google.token_url(),
            ..GoogleEndpoints::google()
        };
        let config = GcpIapConfig {
            name: "iap".parse().unwrap(),
            audience: AUDIENCE.to_owned(),
            relay: Some(RelayClient {
                client_id: GOOGLE_CLIENT.to_owned(),
                client_secret: GOOGLE_SECRET.to_owned(),
            }),
        };
        let clock = world.clock.clock();
        let iap = GcpIapProvider::with_endpoints(config, clock, &base(), endpoints);
        let iap: Arc<dyn AuthProvider> = Arc::new(iap.unwrap());
        let router = app(&world.auth(vec![iap]));
        Setup {
            world,
            google,
            router,
        }
    }

    /// A sign-in begun: the authorization's answer (to Google, with the attempt's cookie).
    struct Begun {
        client: String,
        verifier: String,
        at_google: String,
        cookies: String,
    }

    async fn begin(setup: &Setup, extra: &[(&str, &str)]) -> Begun {
        let client = client_id(&setup.router).await;
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let uri = authorize_uri(&client, REDIRECT, challenge.as_str(), extra);
        let to_google = get(&setup.router, &uri, "").await;
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
        get(&setup.router, &local(&from_google), &begun.cookies).await
    }

    /// A whole sign-in: the client, its verifier, and the code it received.
    async fn signed_in(setup: &Setup) -> (String, String, String) {
        let begun = begin(setup, &[]).await;
        let back = come_back(setup, &begun).await;
        let code = query(&location(&back))["code"].clone();
        (begun.client, begun.verifier, code)
    }

    /// I2: registration is open, answers no secret and both grants (so the client knows it
    /// may refresh), refuses any redirect but loopback, and stores nothing.
    #[tokio::test]
    async fn registration_is_open_loopback_only_and_stores_nothing() {
        let setup = setup().await;
        let registered = json_of(register(&setup.router, REDIRECT).await).await;
        assert!(registered.get("client_secret").is_none(), "{registered}");
        assert_eq!(
            registered["grant_types"],
            json!(["authorization_code", "refresh_token"])
        );
        let web = register(&setup.router, "https://client.example/callback").await;
        assert_eq!(web.status(), StatusCode::BAD_REQUEST);
        assert_eq!(json_of(web).await["error"], "invalid_redirect_uri");
        let later = setup.world.clock.clock().now() + jiff::SignedDuration::from_hours(24 * 3650);
        let expired = setup.world.store.remove_expired(later).await.unwrap();
        assert!(setup.world.log().await.is_empty() && expired == 0);
    }

    /// The client request in Google's `state` as Cairn sent it, with `change` made to it.
    fn changed_state(begun: &Begun, change: impl Fn(&mut Value)) -> String {
        let state = query(&begun.at_google)["state"].clone();
        let mut carried: Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(state).unwrap()).unwrap();
        change(&mut carried);
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&carried).unwrap())
    }

    /// Google's answer for `begun`, carrying `state` (Cairn's own when none is given).
    fn answer_with_state(setup: &Setup, begun: &Begun, state: Option<&str>) -> String {
        let from_google = setup.google.sign_in(&begun.at_google, &ANN);
        let Some(state) = state else {
            return local(&from_google);
        };
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
    /// redirect the client did not register, a state changed on the way, and a callback in
    /// a browser that did not start the attempt are each refused with a page and no
    /// redirect.
    #[tokio::test]
    async fn a_tampered_round_trip_sends_no_code() {
        let setup = setup().await;
        let router = &setup.router;
        let client = client_id(router).await;
        let (challenge, _) = PkceCodeChallenge::new_random_sha256();
        let other = "http://localhost:59411/other";
        let uri = authorize_uri(&client, other, challenge.as_str(), &[]);
        let unregistered = get(router, &uri, "").await;
        assert_eq!(unregistered.status(), StatusCode::BAD_REQUEST);
        assert!(!unregistered.headers().contains_key(LOCATION));

        let begun = begin(&setup, &[]).await;
        let moved = changed_state(&begun, |carried| {
            carried["redirect_uri"] = json!("http://localhost:1/callback");
        });
        let moved = answer_with_state(&setup, &begun, Some(&moved));
        let unchanged = answer_with_state(&setup, &begun, None);
        let cases = [
            ("state changed", moved, begun.cookies.clone()),
            ("another browser", unchanged.clone(), String::new()),
        ];
        for (case, callback, cookies) in cases {
            let refused = get(router, &callback, &cookies).await;
            assert_eq!(refused.status(), StatusCode::BAD_REQUEST, "{case}");
            assert!(!refused.headers().contains_key(LOCATION), "{case}");
        }
        let answered = get(router, &unchanged, &begun.cookies).await;
        assert!(location(&answered).starts_with(REDIRECT));
        assert_eq!(
            answered.headers().get_all(SET_COOKIE).iter().count(),
            1,
            "the cookie is cleared"
        );
    }

    /// No ID token, no success: the token endpoint answers with Google's ID token or fails,
    /// never with Google's access token, and never shows the Google client secret, which
    /// goes only to Google with Cairn's callback as the redirect URI. Google's refusals of a
    /// code or refresh token pass on as `invalid_grant`; a redirect URI the client did not
    /// register is refused before Google is asked.
    #[tokio::test]
    async fn the_token_endpoint_answers_an_id_token_or_fails() {
        let setup = setup().await;
        let router = &setup.router;
        let (client, verifier, code) = signed_in(&setup).await;
        let answered = exchange(router, &client, &verifier, &code).await;
        let text = String::from_utf8(body(answered).await).unwrap();
        assert!(!text.contains(GOOGLE_SECRET), "{text}");
        let answered: Value = serde_json::from_str(&text).unwrap();
        let refresh_token = answered["refresh_token"].as_str().unwrap().to_owned();
        let id_token = answered["access_token"].as_str().unwrap();
        assert!(setup.google.verified(id_token, GOOGLE_CLIENT).is_some());
        let sent = &setup.google.token_forms()[0];
        assert_eq!(sent["client_secret"], GOOGLE_SECRET);
        assert_eq!(sent["redirect_uri"], format!("{ISSUER}/api/oauth/callback"));

        let asked = setup.google.token_forms().len();
        let (client, verifier, code) = signed_in(&setup).await;
        let foreign = [
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", "http://localhost:59411/other"),
            ("client_id", client.as_str()),
            ("code_verifier", verifier.as_str()),
        ];
        let refused = json_of(post_form(router, "/api/oauth/token", "", &foreign).await).await;
        assert_eq!(refused["error"], "invalid_grant");
        assert_eq!(
            setup.google.token_forms().len(),
            asked,
            "Google was not asked"
        );

        let refresh = |token: String| {
            let form = [
                ("grant_type", "refresh_token"),
                ("refresh_token", token.as_str()),
                ("client_id", client.as_str()),
            ];
            let router = router.clone();
            let form: Vec<(String, String)> = form
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect();
            async move {
                let form: Vec<(&str, &str)> = form.iter().map(|(k, v)| (&**k, &**v)).collect();
                post_form(&router, "/api/oauth/token", "", &form).await
            }
        };
        let unknown = refresh("stub-refresh-nope".to_owned()).await;
        assert_eq!(unknown.status(), StatusCode::BAD_REQUEST);
        assert_eq!(json_of(unknown).await["error"], "invalid_grant");

        setup.google.omit_id_token();
        let without = refresh(refresh_token).await;
        assert_eq!(without.status(), StatusCode::BAD_GATEWAY);
        let text = String::from_utf8(body(without).await).unwrap();
        assert!(!text.contains("access_token"), "{text}");
    }

    /// RFC 8707: `resource` is checked against this deployment and never sent to Google: a
    /// foreign one is refused at authorize (at the client, as Cairn) and at the token
    /// endpoint (before Google is asked); the deployment's own passes and goes no further.
    #[tokio::test]
    async fn resource_is_checked_here_and_never_sent_to_google() {
        let setup = setup().await;
        let router = &setup.router;
        let client = client_id(router).await;
        let (challenge, _) = PkceCodeChallenge::new_random_sha256();
        let foreign = [("resource", "https://elsewhere.example/api/mcp")];
        let uri = authorize_uri(&client, REDIRECT, challenge.as_str(), &foreign);
        let back = query(&location(&get(router, &uri, "").await));
        assert_eq!(
            (back["error"].as_str(), back["iss"].as_str()),
            ("invalid_target", ISSUER)
        );

        let own = format!("{ISSUER}/api/mcp");
        let begun = begin(&setup, &[("resource", &own)]).await;
        let code = query(&location(&come_back(&setup, &begun).await))["code"].clone();
        let token = async |resource: &str| {
            let form = [
                ("grant_type", "authorization_code"),
                ("code", code.as_str()),
                ("redirect_uri", REDIRECT),
                ("client_id", begun.client.as_str()),
                ("code_verifier", begun.verifier.as_str()),
                ("resource", resource),
            ];
            post_form(router, "/api/oauth/token", "", &form).await
        };
        let refused = token("https://elsewhere.example/api/mcp").await;
        assert_eq!(json_of(refused).await["error"], "invalid_target");
        assert!(
            setup.google.token_forms().is_empty(),
            "Google was not asked"
        );
        assert_eq!(token(&own).await.status(), StatusCode::OK);
        let at_google = setup.google.authorizations();
        let sent = setup.google.token_forms();
        assert!(!at_google[0].contains_key("resource") && !sent[0].contains_key("resource"));
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
        let request = request("/api/oauth/token", LOCAL)
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

    /// The relay's endpoints are reachable without IAP: once its requests in flight are all
    /// held (by bodies that never arrive), the next is answered 503 at once with
    /// `Retry-After`, and a slot freed is a request served again.
    #[tokio::test]
    async fn past_the_relays_in_flight_limit_a_request_is_answered_503() {
        let setup = setup().await;
        let held: Vec<_> = (0..cairn_auth::limits::RELAY_REQUEST_IN_FLIGHT_COUNT_MAX)
            .map(|_| {
                let stalled = request("/api/oauth/token", LOCAL)
                    .method("POST")
                    .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .header("content-length", "100")
                    .body(Body::from_stream(Stalled))
                    .unwrap();
                let router = setup.router.clone();
                tokio::spawn(async move { send(&router, stalled).await })
            })
            .collect();
        let metadata = "/.well-known/oauth-authorization-server";
        let refused = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let answered = get(&setup.router, metadata, "").await;
                if answered.status() == StatusCode::SERVICE_UNAVAILABLE {
                    return answered;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });
        let refused = refused.await.expect("past the limit, a request is refused");
        assert_eq!(refused.headers()["retry-after"], "1");
        assert_eq!(json_of(refused).await["error"], "temporarily_unavailable");
        let freed = held.into_iter().next().unwrap();
        freed.abort();
        let _ = freed.await;
        assert_eq!(
            get(&setup.router, metadata, "").await.status(),
            StatusCode::OK
        );
    }
}
