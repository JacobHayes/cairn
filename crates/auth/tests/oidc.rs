//! The OIDC provider against a stub issuer on loopback (H1: OAuth/OIDC against a
//! registered provider): a sign-in ends in a session for the identity the ID token names,
//! and every way an ID token or a callback can be wrong is refused (3.2, Security).

#[cfg(test)]
mod support;

#[cfg(test)]
mod oidc {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, OnceLock};

    use axum::Router;
    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::http::header::LOCATION;
    use cairn_auth::{
        AuthProvider, BoxFuture, DevConfig, DevProvider, OidcConfig, OidcProvider, Presented,
        Refusal, Verdict,
    };
    use cairn_schema::{Actor, Identity, Slug, UserId};
    use cairn_store::AuthStore;

    use crate::support::issuer::{Advertise, Issuer, Person, Spoil};
    use crate::support::transcript;
    use crate::support::{
        LOCAL, World, app, base, cookie_header, cookies_set, loopback, request, send, whoami,
        with_cookie,
    };

    const ANN: Person = Person {
        subject: "248289761001",
        name: "Ann Example",
        email: "Ann@Example.org",
        email_verified: true,
    };

    struct Setup {
        world: World,
        issuer: Issuer,
        router: Router,
    }

    async fn setup(auto_link: bool) -> Setup {
        let world = World::new();
        let issuer = Issuer::start(world.clock.clock()).await;
        let config = OidcConfig {
            name: "oidc".parse().unwrap(),
            issuer: issuer.url(),
            client_id: "cairn".to_owned(),
            client_secret: None,
            auto_link,
        };
        let provider = OidcProvider::new(config, world.accounts.clone(), &base()).unwrap();
        let providers: Vec<Arc<dyn AuthProvider>> = vec![Arc::new(provider)];
        let router = app(&world.auth(loopback(), providers));
        Setup {
            world,
            issuer,
            router,
        }
    }

    /// What a browser holds after starting a sign-in: the issuer's authorization URL and
    /// the cookies Cairn set.
    struct Started {
        location: String,
        cookies: String,
    }

    async fn start(setup: &Setup, browser_cookies: &str) -> Started {
        let uri = "/auth/oidc/sign-in?return_to=/journeys";
        let request = with_cookie(request(uri, LOCAL), browser_cookies);
        let response = send(&setup.router, request.body(Body::empty()).unwrap()).await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let location = response.headers()[LOCATION].to_str().unwrap().to_owned();
        Started {
            location,
            cookies: cookie_header(&cookies_set(&response)),
        }
    }

    fn state_of(location: &str) -> String {
        let url: url::Url = location.parse().unwrap();
        let mut pairs = url.query_pairs();
        pairs
            .find(|(name, _)| name == "state")
            .unwrap()
            .1
            .into_owned()
    }

    /// The issuer's callback, from a browser holding `cookies`.
    async fn callback(
        setup: &Setup,
        code: &str,
        state: &str,
        cookies: &str,
    ) -> axum::http::Response<Body> {
        let uri = format!("/auth/oidc/callback?code={code}&state={state}");
        let request = with_cookie(request(&uri, LOCAL), cookies);
        send(&setup.router, request.body(Body::empty()).unwrap()).await
    }

    /// A whole sign-in by `person`, from a browser holding `cookies`: the session cookie it
    /// ends with, or the status it was refused with.
    async fn sign_in(
        setup: &Setup,
        person: &Person,
        spoil: Spoil,
        cookies: &str,
    ) -> Result<String, StatusCode> {
        let started = start(setup, cookies).await;
        let code = setup.issuer.approve(&started.location, person, spoil);
        let held = [cookies, started.cookies.as_str()].join("; ");
        let response = callback(setup, &code, &state_of(&started.location), &held).await;
        if response.status() != StatusCode::SEE_OTHER {
            assert!(
                cookies_set(&response).is_empty(),
                "a refused sign-in sets no cookie"
            );
            return Err(response.status());
        }
        assert_eq!(response.headers()[LOCATION], "/journeys");
        let set = cookies_set(&response);
        let session = set
            .iter()
            .find(|(name, _)| name == "cairn_session")
            .unwrap();
        Ok(format!("cairn_session={}", session.1))
    }

    async fn actor(setup: &Setup, session: &str) -> Actor {
        let request = with_cookie(request("/whoami", LOCAL), session);
        whoami(&setup.router, request.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    /// H1: a sign-in through the issuer ends in a session for a user holding the identity
    /// the ID token names, with its verified email (H3).
    #[tokio::test]
    async fn a_sign_in_ends_in_a_session_for_the_identity_the_issuer_names() {
        let setup = setup(false).await;
        let session = sign_in(&setup, &ANN, Spoil::Nothing, "").await.unwrap();
        let signed_in = actor(&setup, &session).await;
        let held = setup
            .world
            .store
            .identities_of(&signed_in.user)
            .await
            .unwrap();
        let identity = Identity {
            provider: "oidc".parse().unwrap(),
            subject: ANN.subject.parse().unwrap(),
            display: ANN.name.parse().unwrap(),
            verified_emails: [ANN.email.parse().unwrap()].into(),
        };
        transcript::show("The identity linked", &held);
        assert_eq!(held.len(), 1);
        assert_eq!(
            (&held[0].provider, &held[0].subject),
            (&identity.provider, &identity.subject)
        );
        assert_eq!(held[0].verified_emails, identity.verified_emails);
        let user = setup
            .world
            .store
            .user(&signed_in.user)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user.name, identity.display);
        let again = sign_in(&setup, &ANN, Spoil::Nothing, "").await.unwrap();
        assert_eq!(actor(&setup, &again).await, signed_in);
    }

    /// An ID token with the wrong issuer or audience, expired, badly signed, or for another
    /// sign-in's nonce is refused, and no session is started.
    #[tokio::test]
    async fn a_spoiled_id_token_is_refused() {
        let setup = setup(false).await;
        let spoils = [
            Spoil::Issuer,
            Spoil::Audience,
            Spoil::Expired,
            Spoil::Signature,
            Spoil::Nonce,
        ];
        for spoil in spoils {
            transcript::show("An ID token spoiled this way", &spoil);
            let refused = sign_in(&setup, &ANN, spoil, "").await;
            assert_eq!(refused, Err(StatusCode::UNAUTHORIZED), "{spoil:?}");
        }
        assert_eq!(setup.world.log().await, Vec::new());
    }

    /// `state` and the PKCE verifier are bound to the browser that started the sign-in: a
    /// callback without its cookie, with another sign-in's, or replayed is refused.
    #[tokio::test]
    async fn the_state_and_verifier_are_bound_to_the_browser_that_started() {
        let setup = setup(false).await;
        let mine = start(&setup, "").await;
        let theirs = start(&setup, "").await;
        let code = setup.issuer.approve(&mine.location, &ANN, Spoil::Nothing);
        let state = state_of(&mine.location);
        for cookies in ["", theirs.cookies.as_str()] {
            let response = callback(&setup, &code, &state, cookies).await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{cookies:?}");
        }
        let fresh = start(&setup, "").await;
        let code = setup.issuer.approve(&fresh.location, &ANN, Spoil::Nothing);
        let state = state_of(&fresh.location);
        let first = callback(&setup, &code, &state, &fresh.cookies).await;
        assert_eq!(first.status(), StatusCode::SEE_OTHER);
        let replay = callback(&setup, &code, &state, &fresh.cookies).await;
        assert_eq!(replay.status(), StatusCode::BAD_REQUEST);
    }

    /// Signing in while signed in links the identity to the browser's user.
    #[tokio::test]
    async fn signing_in_while_signed_in_links_the_identity() {
        let setup = setup(false).await;
        let tailnet = Identity {
            provider: "tailscale".parse().unwrap(),
            subject: "ann@tailnet".parse().unwrap(),
            display: "Ann".parse().unwrap(),
            verified_emails: std::collections::BTreeSet::new(),
        };
        let user = setup
            .world
            .accounts
            .resolve(&tailnet, None, false)
            .await
            .unwrap();
        let session = setup.world.accounts.start_session(&user).await.unwrap();
        let cookie = format!("cairn_session={}", session.expose());
        let renewed = sign_in(&setup, &ANN, Spoil::Nothing, &cookie)
            .await
            .unwrap();
        assert_eq!(actor(&setup, &renewed).await.user, user);
        assert_eq!(
            setup.world.store.identities_of(&user).await.unwrap().len(),
            2
        );
        let old = with_cookie(request("/whoami", LOCAL), &cookie).body(Body::empty());
        assert_eq!(
            whoami(&setup.router, old.unwrap()).await,
            Err(StatusCode::UNAUTHORIZED)
        );
    }

    /// H3: with auto-link on, a verified email joins the user already holding it; an email
    /// the issuer does not mark verified never links and is not listed.
    #[tokio::test]
    async fn an_unverified_email_never_auto_links() {
        let setup = setup(true).await;
        let holder = Identity {
            provider: "tailscale".parse().unwrap(),
            subject: "ann@tailnet".parse().unwrap(),
            display: "Ann".parse().unwrap(),
            verified_emails: [ANN.email.parse().unwrap()].into(),
        };
        let user = setup
            .world
            .accounts
            .resolve(&holder, None, false)
            .await
            .unwrap();
        let unverified = Person {
            subject: "unverified",
            email_verified: false,
            ..ANN
        };
        let session = sign_in(&setup, &unverified, Spoil::Nothing, "")
            .await
            .unwrap();
        let stranger = actor(&setup, &session).await.user;
        assert_ne!(stranger, user);
        let held = setup.world.store.identities_of(&stranger).await.unwrap();
        assert_eq!(held[0].verified_emails, std::collections::BTreeSet::new());
        let verified = Person {
            subject: "verified",
            ..ANN
        };
        let session = sign_in(&setup, &verified, Spoil::Nothing, "")
            .await
            .unwrap();
        assert_eq!(actor(&setup, &session).await.user, user);
    }
    /// An issuer that cannot be reached fails the sign-in as a bad gateway, and starts
    /// nothing.
    #[tokio::test]
    async fn an_unreachable_issuer_fails_the_sign_in() {
        let world = World::new();
        let config = OidcConfig {
            name: "oidc".parse().unwrap(),
            issuer: "http://127.0.0.1:1/".parse().unwrap(),
            client_id: "cairn".to_owned(),
            client_secret: None,
            auto_link: false,
        };
        let provider = OidcProvider::new(config, world.accounts.clone(), &base()).unwrap();
        let router = app(&world.auth(loopback(), vec![Arc::new(provider)]));
        let start = request("/auth/oidc/sign-in", LOCAL)
            .body(Body::empty())
            .unwrap();
        let response = send(&router, start).await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(cookies_set(&response), Vec::new());
    }
    /// Every call to the issuer goes to https or this machine, whatever its discovery
    /// document advertises, and reads at most the body limit: a key set or token endpoint
    /// over plain http elsewhere, or an oversized document, fails the sign-in.
    #[tokio::test]
    async fn the_issuer_is_called_only_over_https_or_loopback_and_within_the_limit() {
        for advertise in [Advertise::PlainHttpKeys, Advertise::Oversized] {
            let setup = setup(false).await;
            setup.issuer.advertise(advertise);
            let start = request("/auth/oidc/sign-in", LOCAL)
                .body(Body::empty())
                .unwrap();
            let response = send(&setup.router, start).await;
            assert_eq!(response.status(), StatusCode::BAD_GATEWAY, "{advertise:?}");
        }
        let setup = setup(false).await;
        setup.issuer.advertise(Advertise::PlainHttpToken);
        let refused = sign_in(&setup, &ANN, Spoil::Nothing, "").await;
        assert_eq!(refused, Err(StatusCode::BAD_GATEWAY));
    }

    /// A browser another provider already signs in (here the named dev user, as Tailscale
    /// would) links the OIDC identity to that user rather than making a new one.
    #[tokio::test]
    async fn signing_in_while_signed_in_by_another_provider_links_the_identity() {
        let world = World::new();
        let issuer = Issuer::start(world.clock.clock()).await;
        let dev = DevConfig {
            name: "dev".parse().unwrap(),
            user: "Dev".parse().unwrap(),
            verified_emails: std::collections::BTreeSet::new(),
            token: None,
            auto_link: false,
            allow_off_loopback: false,
        };
        let config = OidcConfig {
            name: "oidc".parse().unwrap(),
            issuer: issuer.url(),
            client_id: "cairn".to_owned(),
            client_secret: None,
            auto_link: false,
        };
        let providers: Vec<Arc<dyn AuthProvider>> = vec![
            Arc::new(DevProvider::new(dev, loopback()).unwrap()),
            Arc::new(OidcProvider::new(config, world.accounts.clone(), &base()).unwrap()),
        ];
        let router = app(&world.auth(loopback(), providers));
        let setup = Setup {
            world,
            issuer,
            router,
        };
        let before = whoami(
            &setup.router,
            request("/whoami", LOCAL).body(Body::empty()).unwrap(),
        );
        let dev_user = before.await.unwrap().user;
        sign_in(&setup, &ANN, Spoil::Nothing, "").await.unwrap();
        let identities = setup.world.store.identities_of(&dev_user).await.unwrap();
        let providers: Vec<&str> = identities
            .iter()
            .map(|held| held.provider.as_str())
            .collect();
        assert_eq!(providers, ["dev", "oidc"]);
    }
    async fn session_for(world: &World, subject: &str) -> (UserId, String) {
        let identity = Identity {
            provider: "tailscale".parse().unwrap(),
            subject: subject.parse().unwrap(),
            display: subject.parse().unwrap(),
            verified_emails: std::collections::BTreeSet::new(),
        };
        let user = world
            .accounts
            .resolve(&identity, None, false)
            .await
            .unwrap();
        let session = world.accounts.start_session(&user).await.unwrap();
        (user, format!("cairn_session={}", session.expose()))
    }

    /// The identity links only to the user who started the sign-in: a browser that
    /// changed accounts before the callback is refused, and neither user gains it.
    #[tokio::test]
    async fn a_sign_in_links_only_to_the_user_who_started_it() {
        let setup = setup(false).await;
        let (ann, ann_session) = session_for(&setup.world, "ann@tailnet").await;
        let (bob, bob_session) = session_for(&setup.world, "bob@tailnet").await;
        let started = start(&setup, &ann_session).await;
        let code = setup
            .issuer
            .approve(&started.location, &ANN, Spoil::Nothing);
        let switched = [bob_session.as_str(), started.cookies.as_str()].join("; ");
        let response = callback(&setup, &code, &state_of(&started.location), &switched).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        for user in [&ann, &bob] {
            assert_eq!(
                setup.world.store.identities_of(user).await.unwrap().len(),
                1
            );
        }
    }

    /// A provider that cannot be asked fails the sign-in step, rather than letting it
    /// carry on as if the browser were no one.
    #[tokio::test]
    async fn a_provider_that_cannot_be_asked_fails_the_callback() {
        struct Failing(AtomicBool);
        impl AuthProvider for Failing {
            fn name(&self) -> &Slug {
                static NAME: OnceLock<Slug> = OnceLock::new();
                NAME.get_or_init(|| "failing".parse().unwrap())
            }
            fn authenticate<'a>(&'a self, _: Presented<'a>) -> BoxFuture<'a, Verdict> {
                let verdict = if self.0.load(Ordering::SeqCst) {
                    Verdict::Refused(Refusal::Unavailable("down".to_owned()))
                } else {
                    Verdict::Absent
                };
                Box::pin(async move { verdict })
            }
        }
        let world = World::new();
        let issuer = Issuer::start(world.clock.clock()).await;
        let failing = Arc::new(Failing(AtomicBool::new(false)));
        let config = OidcConfig {
            name: "oidc".parse().unwrap(),
            issuer: issuer.url(),
            client_id: "cairn".to_owned(),
            client_secret: None,
            auto_link: false,
        };
        let providers: Vec<Arc<dyn AuthProvider>> = vec![
            failing.clone(),
            Arc::new(OidcProvider::new(config, world.accounts.clone(), &base()).unwrap()),
        ];
        let router = app(&world.auth(loopback(), providers));
        let setup = Setup {
            world,
            issuer,
            router,
        };
        let started = start(&setup, "").await;
        let code = setup
            .issuer
            .approve(&started.location, &ANN, Spoil::Nothing);
        failing.0.store(true, Ordering::SeqCst);
        let response = callback(
            &setup,
            &code,
            &state_of(&started.location),
            &started.cookies,
        )
        .await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(cookies_set(&response), Vec::new());
        assert_eq!(setup.world.log().await, Vec::new());
    }
    /// An issuer that takes the client secret only in the form post gets it there.
    #[tokio::test]
    async fn the_client_secret_goes_where_the_issuer_takes_it() {
        let world = World::new();
        let issuer = Issuer::start(world.clock.clock()).await;
        issuer.advertise(Advertise::SecretInFormOnly);
        let config = OidcConfig {
            name: "oidc".parse().unwrap(),
            issuer: issuer.url(),
            client_id: "cairn".to_owned(),
            client_secret: Some("shh".to_owned()),
            auto_link: false,
        };
        let provider = OidcProvider::new(config, world.accounts.clone(), &base()).unwrap();
        let router = app(&world.auth(loopback(), vec![Arc::new(provider)]));
        let setup = Setup {
            world,
            issuer,
            router,
        };
        assert!(sign_in(&setup, &ANN, Spoil::Nothing, "").await.is_ok());
    }

    /// A browser the dev user signs in, carrying a lapsed session cookie, still links the
    /// identity to the dev user.
    #[tokio::test]
    async fn a_lapsed_session_does_not_hide_the_provider_that_signs_the_browser_in() {
        let world = World::new();
        let issuer = Issuer::start(world.clock.clock()).await;
        let dev = DevConfig {
            name: "dev".parse().unwrap(),
            user: "Dev".parse().unwrap(),
            verified_emails: std::collections::BTreeSet::new(),
            token: None,
            auto_link: false,
            allow_off_loopback: false,
        };
        let config = OidcConfig {
            name: "oidc".parse().unwrap(),
            issuer: issuer.url(),
            client_id: "cairn".to_owned(),
            client_secret: None,
            auto_link: false,
        };
        let providers: Vec<Arc<dyn AuthProvider>> = vec![
            Arc::new(DevProvider::new(dev, loopback()).unwrap()),
            Arc::new(OidcProvider::new(config, world.accounts.clone(), &base()).unwrap()),
        ];
        let router = app(&world.auth(loopback(), providers));
        let setup = Setup {
            world,
            issuer,
            router,
        };
        let dev_user = whoami(
            &setup.router,
            request("/whoami", LOCAL).body(Body::empty()).unwrap(),
        )
        .await
        .unwrap()
        .user;
        let (_, session) = session_for(&setup.world, "someone@tailnet").await;
        setup
            .world
            .clock
            .advance(i64::try_from(cairn_auth::lifetimes::SESSION_LIFETIME.as_secs()).unwrap());
        sign_in(&setup, &ANN, Spoil::Nothing, &session)
            .await
            .unwrap();
        let providers: Vec<String> = setup
            .world
            .store
            .identities_of(&dev_user)
            .await
            .unwrap()
            .into_iter()
            .map(|held| held.provider.to_string())
            .collect();
        assert_eq!(providers, ["dev", "oidc"]);
    }
}
