//! The Google Cloud IAP provider (H1), offline: assertions signed with ES256 keys generated
//! for the test, verified against a stub of Google's key set on loopback. A good assertion
//! names its user on any listener and from any peer; every spoiled one is refused; a
//! rotated key is fetched, and forged key ids do not fetch on every request (3.2, Security).
//! With the relay on, a Google ID token as the bearer counts only beside a valid assertion
//! (I2); without it, the provider is as it was.

#[cfg(test)]
mod gcp_iap {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::http::header::{CONTENT_TYPE, WWW_AUTHENTICATE};
    use cairn_auth::gcp_iap::ASSERTION_HEADER;
    use cairn_auth::{
        AuthProvider, DevConfig, DevProvider, GcpIapConfig, GcpIapProvider, GoogleEndpoints, Peer,
        Presented, Refusal, RelayClient, Verdict,
    };
    use cairn_store::AuthStore;
    use serde_json::{Value, json};
    use url::Url;

    use crate::auth::support::iap::{AUDIENCE, KeySet, claims_of, key, sign, sign_as};
    use crate::auth::support::{
        LOCAL, REMOTE, World, app, base, loopback, public, request, send, whoami,
    };

    const SUBJECT: &str = "accounts.google.com:1234567890";
    const EMAIL: &str = "ann@example.org";

    fn config() -> GcpIapConfig {
        GcpIapConfig {
            name: "iap".parse().unwrap(),
            audience: AUDIENCE.to_owned(),
            auto_link: false,
            relay: None,
        }
    }

    fn relay_config() -> GcpIapConfig {
        GcpIapConfig {
            relay: Some(RelayClient {
                client_id: "relay-client".to_owned(),
                client_secret: "relay-secret".to_owned(),
            }),
            ..config()
        }
    }

    /// Google's endpoints, but the IAP key set at `url`.
    fn endpoints(url: &Url) -> GoogleEndpoints {
        GoogleEndpoints {
            iap_key_set: url.clone(),
            ..GoogleEndpoints::google()
        }
    }

    fn provider_with(world: &World, keys: &KeySet, config: GcpIapConfig) -> GcpIapProvider {
        let clock = world.clock.clock();
        GcpIapProvider::with_endpoints(config, clock, &base(), endpoints(&keys.url)).unwrap()
    }

    fn provider(world: &World, keys: &KeySet) -> GcpIapProvider {
        provider_with(world, keys, config())
    }

    /// The claims IAP signs for Ann, issued at `now`.
    fn claims(now: i64) -> Value {
        claims_of(SUBJECT, EMAIL, now)
    }

    async fn verdict(provider: &GcpIapProvider, assertion: Option<&str>) -> Verdict {
        let mut headers = axum::http::HeaderMap::new();
        if let Some(assertion) = assertion {
            headers.insert(ASSERTION_HEADER, assertion.parse().unwrap());
        }
        let presented = Presented {
            headers: &headers,
            peer: Peer::Unknown,
        };
        provider.authenticate(presented).await
    }

    fn now(world: &World) -> i64 {
        world.clock.clock().now().as_second()
    }

    /// A good assertion names its user through the layer, on a listener every machine
    /// reaches and from a remote peer: the signature, not the network, is the credential.
    /// The subject is IAP's account id and the email is listed as verified (H3).
    #[tokio::test]
    async fn a_signed_assertion_names_its_user_on_any_listener() {
        let signer = key("one", 7);
        let keys = KeySet::start(&[&signer]).await;
        let world = World::new();
        let iap: Arc<dyn AuthProvider> = Arc::new(provider(&world, &keys));
        let router = app(&world.auth(public(), vec![iap]));
        let assertion = sign(&claims(now(&world)), &signer);
        let get = request("/whoami", REMOTE)
            .header(ASSERTION_HEADER, &assertion)
            .body(Body::empty())
            .unwrap();
        let ann = whoami(&router, get).await.unwrap();
        let logins = world.store.logins_of(&ann.user).await.unwrap();
        assert_eq!(logins[0].subject.as_str(), SUBJECT);
        assert!(logins[0].verified_emails.contains(&EMAIL.parse().unwrap()));
        let again = request("/whoami", REMOTE)
            .header(ASSERTION_HEADER, &assertion)
            .body(Body::empty())
            .unwrap();
        assert_eq!(whoami(&router, again).await.unwrap().user, ann.user);
        assert_eq!(keys.fetches(), 1, "the key set is kept between requests");
    }

    /// Without the header the provider reads nothing; with it, every spoiled assertion is
    /// refused as a bad credential, never passed to another provider.
    #[tokio::test]
    async fn every_spoiled_assertion_is_refused() {
        let signer = key("one", 7);
        let impostor = key("one", 8);
        let keys = KeySet::start(&[&signer]).await;
        let world = World::new();
        let iap = provider(&world, &keys);
        let at = now(&world);
        assert_eq!(verdict(&iap, None).await, Verdict::Absent);
        let good = verdict(&iap, Some(&sign(&claims(at), &signer))).await;
        assert!(matches!(good, Verdict::Login(_)), "{good:?}");

        let with = |field: &str, value: Value| {
            let mut claims = claims(at);
            claims[field] = value;
            claims
        };
        let skew =
            i64::try_from(cairn_auth::limits::IAP_ASSERTION_CLOCK_SKEW_MAX.as_secs()).unwrap();
        let within_skew = sign(&with("exp", json!(at - skew + 1)), &signer);
        let within_skew = verdict(&iap, Some(&within_skew)).await;
        assert!(matches!(within_skew, Verdict::Login(_)), "{within_skew:?}");
        let mut tampered = sign(&claims(at), &signer);
        tampered.replace_range(tampered.len() - 4.., "AAAA");
        let cases = [
            (
                "audience",
                sign(
                    &with("aud", json!("/projects/1/global/backendServices/2")),
                    &signer,
                ),
            ),
            (
                "issuer",
                sign(&with("iss", json!("https://accounts.google.com")), &signer),
            ),
            ("expired", sign(&with("exp", json!(at - skew - 1)), &signer)),
            (
                "issued later",
                sign(&with("iat", json!(at + skew + 60)), &signer),
            ),
            ("other key", sign(&claims(at), &impostor)),
            ("tampered", tampered),
            ("algorithm", sign_as(&claims(at), "one", "ES384", &signer)),
            ("not a token", "not.a.token".to_owned()),
            ("empty", " ".to_owned()),
        ];
        for (case, assertion) in cases {
            assert_eq!(
                verdict(&iap, Some(&assertion)).await,
                Verdict::Refused(Refusal::Credential),
                "{case}"
            );
        }
    }

    /// Google rotates its keys: an assertion under a new key id fetches the set again and
    /// passes. A forged key id is refused, and fetches again only once the refetch interval
    /// has passed since the last fetch.
    #[tokio::test]
    async fn a_new_key_is_fetched_but_forged_key_ids_do_not_fetch_every_time() {
        let old = key("old", 7);
        let new = key("new", 9);
        let keys = KeySet::start(&[&old]).await;
        let world = World::new();
        let iap = provider(&world, &keys);
        let good = verdict(&iap, Some(&sign(&claims(now(&world)), &old))).await;
        assert!(matches!(good, Verdict::Login(_)), "{good:?}");
        assert_eq!(keys.fetches(), 1);

        keys.publish(&[&old, &new]);
        world.clock.advance(61);
        let rotated = verdict(&iap, Some(&sign(&claims(now(&world)), &new))).await;
        assert!(matches!(rotated, Verdict::Login(_)), "{rotated:?}");
        assert_eq!(keys.fetches(), 2);

        let forged = key("forged", 11);
        for _ in 0..3 {
            let refused = verdict(&iap, Some(&sign(&claims(now(&world)), &forged))).await;
            assert_eq!(refused, Verdict::Refused(Refusal::Credential));
        }
        assert_eq!(keys.fetches(), 2, "within the interval, no fetch");
        world.clock.advance(61);
        let refused = verdict(&iap, Some(&sign(&claims(now(&world)), &forged))).await;
        assert_eq!(refused, Verdict::Refused(Refusal::Credential));
        assert_eq!(keys.fetches(), 3, "past the interval, one fetch");
    }

    /// When the key set cannot be fetched, the assertion cannot be checked: refused as
    /// unavailable, not as a bad credential.
    #[tokio::test]
    async fn an_unreachable_key_set_refuses_as_unavailable() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url: Url = format!("http://{}/keys", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        drop(listener);
        let world = World::new();
        let clock = world.clock.clock();
        let iap = GcpIapProvider::with_endpoints(config(), clock, &base(), endpoints(&url));
        let iap = iap.unwrap();
        let signer = key("one", 7);
        let refused = verdict(&iap, Some(&sign(&claims(now(&world)), &signer))).await;
        assert!(
            matches!(refused, Verdict::Refused(Refusal::Unavailable(_))),
            "{refused:?}"
        );
    }

    /// The provider refuses to start with an audience that is not an IAP resource, a Google
    /// endpoint Cairn may not call, or a relay client with an empty or padded id or secret.
    #[test]
    fn a_bad_audience_or_key_set_refuses_to_start() {
        let world = World::new();
        let made = |config: GcpIapConfig, endpoints: GoogleEndpoints| {
            GcpIapProvider::with_endpoints(config, world.clock.clock(), &base(), endpoints)
        };
        for audience in ["", "123456789", " /projects/1/global/backendServices/2"] {
            let config = GcpIapConfig {
                audience: audience.to_owned(),
                ..config()
            };
            assert!(
                made(config, GoogleEndpoints::google()).is_err(),
                "{audience:?}"
            );
        }
        let plain: Url = "http://google.example/x".parse().unwrap();
        let google = GoogleEndpoints::google;
        for endpoints in [
            GoogleEndpoints {
                iap_key_set: plain.clone(),
                ..google()
            },
            GoogleEndpoints {
                authorization: plain.clone(),
                ..google()
            },
            GoogleEndpoints {
                token: plain.clone(),
                ..google()
            },
        ] {
            assert!(
                made(relay_config(), endpoints.clone()).is_err(),
                "{endpoints:?}"
            );
        }
        for (client_id, client_secret) in [("", "secret"), ("id", ""), (" id", "secret")] {
            let relay = RelayClient {
                client_id: client_id.to_owned(),
                client_secret: client_secret.to_owned(),
            };
            let config = GcpIapConfig {
                relay: Some(relay),
                ..config()
            };
            assert!(
                made(config, google()).is_err(),
                "{client_id:?} {client_secret:?}"
            );
        }
        assert!(GcpIapProvider::new(relay_config(), world.clock.clock(), &base()).is_ok());
        assert!(GcpIapProvider::new(config(), world.clock.clock(), &base()).is_ok());
    }

    /// A Google ID token, as the relay hands one to a client: IAP checks it; Cairn only
    /// sees its shape.
    fn google_bearer(world: &World) -> String {
        let at = now(world);
        let claims = json!({
            "iss": "https://accounts.google.com",
            "aud": "relay-client",
            "sub": "1098765432",
            "email": "bob@example.org",
            "iat": at,
            "exp": at + 3600,
        });
        format!("Bearer {}", sign(&claims, &key("google", 3)))
    }

    /// H1, I2: with the relay on, a Google bearer counts only beside a valid assertion on
    /// the same request, and the assertion names the user; alone, or beside a bad
    /// assertion, it is refused as a bad credential, never left for another provider. An
    /// assertion alone (a browser behind IAP) signs its user in as before.
    #[tokio::test]
    async fn a_google_bearer_counts_only_beside_a_valid_assertion() {
        let signer = key("one", 7);
        let impostor = key("one", 8);
        let keys = KeySet::start(&[&signer]).await;
        let world = World::new();
        let iap: Arc<dyn AuthProvider> = Arc::new(provider_with(&world, &keys, relay_config()));
        let router = app(&world.auth(public(), vec![iap]));
        let bearer = google_bearer(&world);
        let at = now(&world);
        let expired = sign(&claims_of(SUBJECT, EMAIL, at - 3600), &signer);
        let call = |assertion: Option<&str>, bearer: Option<&str>| {
            let mut call = request("/whoami", REMOTE);
            if let Some(assertion) = assertion {
                call = call.header(ASSERTION_HEADER, assertion);
            }
            if let Some(bearer) = bearer {
                call = call.header("authorization", bearer);
            }
            call.body(Body::empty()).unwrap()
        };
        for (case, assertion) in [
            ("no assertion", None),
            (
                "another key's assertion",
                Some(sign(&claims(at), &impostor)),
            ),
            ("an expired assertion", Some(expired)),
        ] {
            let refused = send(&router, call(assertion.as_deref(), Some(&bearer))).await;
            assert_eq!(refused.status(), StatusCode::UNAUTHORIZED, "{case}");
            let challenge = refused.headers()[WWW_AUTHENTICATE].to_str().unwrap();
            assert!(
                challenge.contains("error=\"invalid_token\""),
                "{case}: {challenge}"
            );
        }
        let assertion = sign(&claims(at), &signer);
        let ann = whoami(&router, call(Some(&assertion), Some(&bearer)))
            .await
            .unwrap();
        let logins = world.store.logins_of(&ann.user).await.unwrap();
        assert_eq!(logins[0].subject.as_str(), SUBJECT);
        let browser = whoami(&router, call(Some(&assertion), None)).await.unwrap();
        assert_eq!(browser, ann);
    }

    /// A refused Google bearer never falls through to a provider that would sign the
    /// request in regardless (a tokenless dev user on this machine).
    #[tokio::test]
    async fn a_lone_google_bearer_never_falls_through_to_another_provider() {
        let keys = KeySet::start(&[&key("one", 7)]).await;
        let world = World::new();
        let dev = DevConfig {
            name: "dev".parse().unwrap(),
            user: "Dev".parse().unwrap(),
            verified_emails: ["dev@example.org".parse().unwrap()].into(),
            token: None,
            auto_link: false,
            allow_off_loopback: false,
        };
        let providers: Vec<Arc<dyn AuthProvider>> = vec![
            Arc::new(DevProvider::new(dev, loopback()).unwrap()),
            Arc::new(provider_with(&world, &keys, relay_config())),
        ];
        let router = app(&world.auth(loopback(), providers));
        let plain = request("/whoami", LOCAL).body(Body::empty()).unwrap();
        assert!(
            whoami(&router, plain).await.is_ok(),
            "the dev user signs in"
        );
        let bearer = request("/whoami", LOCAL).header("authorization", google_bearer(&world));
        let refused = whoami(&router, bearer.body(Body::empty()).unwrap()).await;
        assert_eq!(refused, Err(StatusCode::UNAUTHORIZED));
    }

    /// Without a Google client the provider is as it was: it claims no bearer (so one is
    /// refused even beside a good assertion), serves no OAuth endpoints, and names no
    /// resource metadata in its challenge.
    #[tokio::test]
    async fn without_a_client_gcp_iap_is_as_before() {
        let signer = key("one", 7);
        let keys = KeySet::start(&[&signer]).await;
        let world = World::new();
        let iap: Arc<dyn AuthProvider> = Arc::new(provider(&world, &keys));
        let router = app(&world.auth(public(), vec![iap]));
        let assertion = sign(&claims(now(&world)), &signer);
        let both = request("/whoami", REMOTE)
            .header(ASSERTION_HEADER, &assertion)
            .header("authorization", google_bearer(&world));
        let refused = whoami(&router, both.body(Body::empty()).unwrap()).await;
        assert_eq!(refused, Err(StatusCode::UNAUTHORIZED));
        let register = request("/api/oauth/register", REMOTE)
            .method("POST")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"redirect_uris":["http://localhost:1/cb"]}"#))
            .unwrap();
        assert_eq!(
            send(&router, register).await.status(),
            StatusCode::NOT_FOUND
        );
        let metadata = request("/.well-known/oauth-protected-resource/api/mcp", REMOTE);
        let metadata = send(&router, metadata.body(Body::empty()).unwrap()).await;
        assert_eq!(metadata.status(), StatusCode::NOT_FOUND);
        let anonymous = request("/whoami", REMOTE).body(Body::empty()).unwrap();
        let anonymous = send(&router, anonymous).await;
        let challenge = anonymous.headers()[WWW_AUTHENTICATE].to_str().unwrap();
        assert!(!challenge.contains("resource_metadata"), "{challenge}");
    }
}
