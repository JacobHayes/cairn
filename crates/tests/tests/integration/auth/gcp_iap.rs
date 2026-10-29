//! The Google Cloud IAP provider (H1), offline: assertions signed with ES256 keys generated
//! for the test, verified against a stub of Google's key set on loopback. A good assertion
//! names its user on any listener and from any peer; every spoiled one is refused, one
//! lasting longer than IAP's do included; a rotated key is fetched, a withdrawn one stops
//! verifying once the kept set ages out, and neither forged key ids nor failed fetches
//! fetch on every request, nor does a slow fetch hold up requests whose key is kept.
//! With the relay on, a Google ID token as the bearer counts only beside a valid assertion
//! (I2); without it, the provider is as it was. Only a Google account's email counts as
//! verified, so an external identity is refused, and so is a request with several
//! assertions.

#[cfg(test)]
mod gcp_iap {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::http::header::WWW_AUTHENTICATE;
    use cairn_auth::gcp_iap::ASSERTION_HEADER;
    use cairn_auth::{
        AuthProvider, GcpIapConfig, GcpIapProvider, GoogleEndpoints, Presented, Refusal,
        RelayClient, Verdict,
    };
    use cairn_store::AuthStore;
    use serde_json::{Value, json};
    use url::Url;

    use crate::auth::support::iap::{AUDIENCE, KeySet, claims_of, key, sign};
    use crate::auth::support::{REMOTE, World, app, base, request, send, whoami};

    const SUBJECT: &str = "accounts.google.com:1234567890";
    const EMAIL: &str = "ann@example.org";

    fn config() -> GcpIapConfig {
        GcpIapConfig {
            name: "iap".parse().unwrap(),
            audience: AUDIENCE.to_owned(),
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
            peer: None,
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
        let router = app(&world.auth(vec![iap]));
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
        let lifetime_max =
            i64::try_from(cairn_auth::limits::IAP_ASSERTION_LIFETIME_MAX.as_secs()).unwrap();
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
            ("expired", sign(&with("exp", json!(at - skew - 1)), &signer)),
            (
                "lasts too long",
                sign(
                    &with("exp", json!(at + lifetime_max + 2 * skew + 1)),
                    &signer,
                ),
            ),
            ("other key", sign(&claims(at), &impostor)),
            ("tampered", tampered),
        ];
        for (case, assertion) in cases {
            assert_eq!(
                verdict(&iap, Some(&assertion)).await,
                Verdict::Refused(Refusal::Credential),
                "{case}"
            );
        }
    }

    /// IAP sends one assertion; a request carrying several is refused, even when the first
    /// is good, rather than believing one of them.
    #[tokio::test]
    async fn a_repeated_assertion_header_is_refused() {
        let signer = key("one", 7);
        let keys = KeySet::start(&[&signer]).await;
        let world = World::new();
        let iap = provider(&world, &keys);
        let good = sign(&claims(now(&world)), &signer);
        let other = sign(
            &claims_of("accounts.google.com:2", "bob@example.org", now(&world)),
            &signer,
        );
        let mut headers = axum::http::HeaderMap::new();
        headers.append(ASSERTION_HEADER, good.parse().unwrap());
        headers.append(ASSERTION_HEADER, other.parse().unwrap());
        let presented = Presented {
            headers: &headers,
            peer: None,
        };
        let refused = iap.authenticate(presented).await;
        assert_eq!(refused, Verdict::Refused(Refusal::Credential));
    }

    /// H3: a Google account's email is verified by Google; an Identity Platform (external)
    /// identity's is vouched for by its own provider, so it is not listed as verified, and
    /// with no verified email the identity is refused rather than matched to the user who
    /// signed in with that email through Google.
    #[tokio::test]
    async fn an_external_identity_is_refused_not_matched_by_email() {
        let signer = key("one", 7);
        let keys = KeySet::start(&[&signer]).await;
        let world = World::new();
        let iap: Arc<dyn AuthProvider> = Arc::new(provider(&world, &keys));
        let router = app(&world.auth(vec![iap]));
        let call = |subject: &str| {
            let assertion = sign(&claims_of(subject, EMAIL, now(&world)), &signer);
            let call = request("/whoami", REMOTE).header(ASSERTION_HEADER, assertion);
            call.body(Body::empty()).unwrap()
        };
        assert!(
            whoami(&router, call(SUBJECT)).await.is_ok(),
            "Ann, through Google"
        );
        let external = "securetoken.google.com/project-1/tenant-1:gZG0yELPypZElTmAT9I55prjHg63";
        let refused = whoami(&router, call(external)).await;
        assert_eq!(refused, Err(StatusCode::FORBIDDEN));
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
        world.clock.advance(refetch_interval() + 1);
        let rotated = verdict(&iap, Some(&sign(&claims(now(&world)), &new))).await;
        assert!(matches!(rotated, Verdict::Login(_)), "{rotated:?}");
        assert_eq!(keys.fetches(), 2);

        let forged = key("forged", 11);
        for _ in 0..3 {
            let refused = verdict(&iap, Some(&sign(&claims(now(&world)), &forged))).await;
            assert_eq!(refused, Verdict::Refused(Refusal::Credential));
        }
        assert_eq!(keys.fetches(), 2, "within the interval, no fetch");
        world.clock.advance(refetch_interval() + 1);
        let refused = verdict(&iap, Some(&sign(&claims(now(&world)), &forged))).await;
        assert_eq!(refused, Verdict::Refused(Refusal::Credential));
        assert_eq!(keys.fetches(), 3, "past the interval, one fetch");
    }

    /// Google withdraws a key: the kept set is trusted until it ages out, then fetched
    /// again, and the withdrawn key no longer verifies while the one still published does.
    #[tokio::test]
    async fn a_withdrawn_key_stops_verifying_once_the_kept_set_ages_out() {
        let old = key("old", 7);
        let new = key("new", 9);
        let keys = KeySet::start(&[&old, &new]).await;
        let world = World::new();
        let iap = provider(&world, &keys);
        let good = verdict(&iap, Some(&sign(&claims(now(&world)), &old))).await;
        assert!(matches!(good, Verdict::Login(_)), "{good:?}");

        keys.publish(&[&new]);
        let age_max = i64::try_from(cairn_auth::limits::IAP_KEY_SET_AGE_MAX.as_secs()).unwrap();
        world.clock.advance(age_max - 1);
        let kept = verdict(&iap, Some(&sign(&claims(now(&world)), &old))).await;
        assert!(matches!(kept, Verdict::Login(_)), "{kept:?}");
        assert_eq!(keys.fetches(), 1, "within the age, the kept set verifies");
        world.clock.advance(1);
        let withdrawn = verdict(&iap, Some(&sign(&claims(now(&world)), &old))).await;
        assert_eq!(withdrawn, Verdict::Refused(Refusal::Credential));
        assert_eq!(keys.fetches(), 2, "past the age, one fetch");
        let published = verdict(&iap, Some(&sign(&claims(now(&world)), &new))).await;
        assert!(matches!(published, Verdict::Login(_)), "{published:?}");
        assert_eq!(keys.fetches(), 2);
    }

    /// A failed fetch holds off the next as a good one does, both before any set is kept
    /// and when the kept set has aged out: while Google is down, Cairn fetches once per
    /// interval, not once per request, and refuses as unavailable rather than trusting an
    /// aged set.
    #[tokio::test]
    async fn failed_fetches_hold_off_the_next_as_good_ones_do() {
        let signer = key("one", 7);
        let keys = KeySet::start(&[&signer]).await;
        let world = World::new();
        let iap = provider(&world, &keys);
        let unavailable = async |iap: &GcpIapProvider, world: &World| {
            for _ in 0..3 {
                let refused = verdict(iap, Some(&sign(&claims(now(world)), &signer))).await;
                assert!(
                    matches!(refused, Verdict::Refused(Refusal::Unavailable(_))),
                    "{refused:?}"
                );
            }
        };
        keys.fail(true);
        unavailable(&iap, &world).await;
        assert_eq!(keys.fetches(), 1, "first fetches: one try per interval");

        keys.fail(false);
        world.clock.advance(refetch_interval() + 1);
        let good = verdict(&iap, Some(&sign(&claims(now(&world)), &signer))).await;
        assert!(matches!(good, Verdict::Login(_)), "{good:?}");
        assert_eq!(keys.fetches(), 2);

        keys.fail(true);
        let age_max = i64::try_from(cairn_auth::limits::IAP_KEY_SET_AGE_MAX.as_secs()).unwrap();
        world.clock.advance(age_max);
        unavailable(&iap, &world).await;
        assert_eq!(keys.fetches(), 3, "refreshes: one try per interval");
    }

    /// The refetch interval, in seconds.
    fn refetch_interval() -> i64 {
        i64::try_from(cairn_auth::limits::IAP_KEY_SET_REFETCH_INTERVAL_MIN.as_secs()).unwrap()
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

    /// The provider refuses to start with an audience that is not an IAP resource or a
    /// Google endpoint Cairn may not call.
    #[test]
    fn a_bad_audience_or_key_set_refuses_to_start() {
        let world = World::new();
        let made = |config: GcpIapConfig, endpoints: GoogleEndpoints| {
            GcpIapProvider::with_endpoints(config, world.clock.clock(), &base(), endpoints)
        };
        for audience in ["", "123456789"] {
            let config = GcpIapConfig {
                audience: audience.to_owned(),
                ..config()
            };
            assert!(
                made(config, GoogleEndpoints::google()).is_err(),
                "{audience:?}"
            );
        }
        let plain = GoogleEndpoints {
            iap_key_set: "http://google.example/x".parse().unwrap(),
            ..GoogleEndpoints::google()
        };
        assert!(made(config(), plain).is_err());
        assert!(made(relay_config(), GoogleEndpoints::google()).is_ok());
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
        let router = app(&world.auth(vec![iap]));
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
}
