//! The OIDC provider against a stub issuer on loopback (H1: OAuth/OIDC against a
//! registered provider): a sign-in ends in a session for the login the ID token names,
//! and every way an ID token or a callback can be wrong is refused (3.2, Security).

#[cfg(test)]
mod oidc {
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::http::header::LOCATION;
    use cairn_auth::{AuthProvider, OidcConfig, OidcProvider};
    use cairn_schema::{Actor, Login};
    use cairn_store::{AuthLogQuery, AuthStore};

    use crate::auth::support::issuer::{Advertise, Issuer, Person, Spoil};
    use crate::auth::support::{
        LOCAL, World, app, base, cookie_header, cookies_set, request, send, whoami, with_cookie,
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

    async fn setup() -> Setup {
        let world = World::new();
        let issuer = Issuer::start(world.clock.clock()).await;
        let config = OidcConfig {
            name: "oidc".parse().unwrap(),
            issuer: issuer.url(),
            client_id: "cairn".to_owned(),
            client_secret: None,
        };
        let provider = OidcProvider::new(config, world.accounts.clone(), &base()).unwrap();
        let providers: Vec<Arc<dyn AuthProvider>> = vec![Arc::new(provider)];
        let router = app(&world.auth(providers));
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
        let uri = "/api/auth/oidc/sign-in?return_to=/journeys%3Fmine%3D1";
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
        let uri = format!("/api/auth/oidc/callback?code={code}&state={state}");
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
        assert_eq!(response.headers()[LOCATION], "/journeys?mine=1");
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

    /// H1: a sign-in through the issuer ends in a session for a user holding the login
    /// the ID token names, with its verified email (H3).
    #[tokio::test]
    async fn a_sign_in_ends_in_a_session_for_the_login_the_issuer_names() {
        let setup = setup().await;
        let session = sign_in(&setup, &ANN, Spoil::Nothing, "").await.unwrap();
        let signed_in = actor(&setup, &session).await;
        let held = setup.world.store.logins_of(&signed_in.user).await.unwrap();
        let login = Login {
            provider: "oidc".parse().unwrap(),
            subject: ANN.subject.parse().unwrap(),
            name: Some(ANN.name.parse().unwrap()),
            verified_emails: [ANN.email.parse().unwrap()].into(),
        };
        assert_eq!(held.len(), 1);
        assert_eq!(
            (&held[0].provider, &held[0].subject),
            (&login.provider, &login.subject)
        );
        assert_eq!(held[0].verified_emails, login.verified_emails);
        assert_eq!(held[0].name, login.name);
        let again = sign_in(&setup, &ANN, Spoil::Nothing, "").await.unwrap();
        assert_eq!(actor(&setup, &again).await, signed_in);
    }

    /// An ID token with the wrong issuer or audience, expired, badly signed, or for another
    /// sign-in's nonce is refused, and no session is started.
    #[tokio::test]
    async fn a_spoiled_id_token_is_refused() {
        let setup = setup().await;
        let spoils = [
            Spoil::Issuer,
            Spoil::Audience,
            Spoil::Expired,
            Spoil::Signature,
            Spoil::Nonce,
        ];
        for spoil in spoils {
            let refused = sign_in(&setup, &ANN, spoil, "").await;
            assert_eq!(refused, Err(StatusCode::UNAUTHORIZED), "{spoil:?}");
        }
        assert_eq!(setup.world.log().await, Vec::new());
    }

    /// `state` and the PKCE verifier are bound to the browser that started the sign-in: a
    /// callback without its cookie, with another sign-in's, or replayed is refused.
    #[tokio::test]
    async fn the_state_and_verifier_are_bound_to_the_browser_that_started() {
        let setup = setup().await;
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

    /// H3: an email the issuer does not mark verified signs no one in: a sign-in with no
    /// verified email is refused, starting no session and creating no user.
    #[tokio::test]
    async fn an_unverified_email_signs_no_one_in() {
        let setup = setup().await;
        let unverified = Person {
            subject: "unverified",
            email_verified: false,
            ..ANN
        };
        let refused = sign_in(&setup, &unverified, Spoil::Nothing, "").await;
        assert_eq!(refused, Err(StatusCode::FORBIDDEN));
        let query = AuthLogQuery::default();
        let logged = setup.world.accounts.auth_log(&query).await.unwrap();
        assert_eq!(logged.items, Vec::new(), "no user or login was added");
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
        };
        let provider = OidcProvider::new(config, world.accounts.clone(), &base()).unwrap();
        let router = app(&world.auth(vec![Arc::new(provider)]));
        let start = request("/api/auth/oidc/sign-in", LOCAL)
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
            let setup = setup().await;
            setup.issuer.advertise(advertise);
            let start = request("/api/auth/oidc/sign-in", LOCAL)
                .body(Body::empty())
                .unwrap();
            let response = send(&setup.router, start).await;
            assert_eq!(response.status(), StatusCode::BAD_GATEWAY, "{advertise:?}");
        }
        let setup = setup().await;
        setup.issuer.advertise(Advertise::PlainHttpToken);
        let refused = sign_in(&setup, &ANN, Spoil::Nothing, "").await;
        assert_eq!(refused, Err(StatusCode::BAD_GATEWAY));
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
        };
        let provider = OidcProvider::new(config, world.accounts.clone(), &base()).unwrap();
        let router = app(&world.auth(vec![Arc::new(provider)]));
        let setup = Setup {
            world,
            issuer,
            router,
        };
        assert!(sign_in(&setup, &ANN, Spoil::Nothing, "").await.is_ok());
    }
}
