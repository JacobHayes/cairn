//! The auth layer on a bare axum router (readiness ruling: 4.2's API does not exist yet),
//! with the dev provider and browser sessions: H1 (a simple local mode), and the security
//! rules a request meets whatever provider it reaches (3.2, Security).

#[cfg(test)]
mod layer {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
    use cairn_auth::lifetimes::SESSION_LIFETIME;
    use cairn_auth::{AuthProvider, DevConfig, DevProvider, Listener};
    use cairn_store::{AuthEvent, AuthStore};

    use crate::support::transcript;
    use crate::support::{
        LOCAL, REMOTE, World, app, cookie_header, cookies_set, loopback, public, request, send,
        whoami, with_cookie,
    };

    fn dev(name: &str, token: Option<&str>) -> DevConfig {
        DevConfig {
            name: name.parse().unwrap(),
            user: format!("{name} user").parse().unwrap(),
            verified_emails: BTreeSet::new(),
            token: token.map(str::to_owned),
            auto_link: false,
            allow_off_loopback: false,
        }
    }

    fn provider(config: DevConfig, listener: Listener) -> Arc<dyn AuthProvider> {
        Arc::new(DevProvider::new(config, listener).unwrap())
    }

    fn get(uri: &str, peer: &str) -> axum::http::Request<Body> {
        request(uri, peer).body(Body::empty()).unwrap()
    }

    fn bearer(token: &str, peer: &str) -> axum::http::Request<Body> {
        let builder = request("/whoami", peer).header(AUTHORIZATION, format!("Bearer {token}"));
        builder.body(Body::empty()).unwrap()
    }

    /// H1: the named dev user signs every local request in as one user, created on the
    /// first and logged; later requests find the same user.
    #[tokio::test]
    async fn the_named_dev_user_is_one_user_created_once() {
        let world = World::new();
        let auth = world.auth(loopback(), vec![provider(dev("dev", None), loopback())]);
        let router = app(&auth);
        let first = whoami(&router, get("/whoami", LOCAL)).await.unwrap();
        let again = whoami(&router, get("/whoami", LOCAL)).await.unwrap();
        assert_eq!(first, again);
        assert_eq!(first.agent, None);
        let user = world.store.user(&first.user).await.unwrap().unwrap();
        transcript::show("The user created", &user);
        assert_eq!(user.name.as_str(), "dev user");
        let logged = [AuthEvent::UserCreated, AuthEvent::IdentityLinked];
        assert_eq!(world.log().await, logged);
        transcript::show("The auth log", &logged);
    }

    /// Dev mode is refused off loopback without the override: it does not start on a
    /// listener other machines reach, and refuses a request from another machine.
    #[tokio::test]
    async fn dev_mode_is_refused_off_loopback_without_the_override() {
        let unix = DevProvider::new(dev("dev", None), Listener::Unix);
        assert!(unix.is_ok());
        let refused = DevProvider::new(dev("dev", None), public());
        transcript::show(
            "The dev provider on 0.0.0.0:8080 without the override",
            &refused,
        );
        assert!(refused.is_err());
        let overridden = DevConfig {
            allow_off_loopback: true,
            ..dev("dev", None)
        };
        let world = World::new();
        let open = world.auth(public(), vec![provider(overridden, public())]);
        assert!(whoami(&app(&open), get("/whoami", REMOTE)).await.is_ok());

        let local = world.auth(loopback(), vec![provider(dev("dev", None), loopback())]);
        let status = whoami(&app(&local), get("/whoami", REMOTE)).await;
        assert_eq!(status, Err(StatusCode::FORBIDDEN));
    }

    /// The static token signs in as the dev user; a wrong token is refused with
    /// `invalid_token`, no credential at all asks for one, and a bearer token no provider
    /// claims is refused.
    #[tokio::test]
    async fn the_dev_token_signs_in_and_a_wrong_or_unclaimed_token_is_refused() {
        let world = World::new();
        let config = dev("dev", Some("let-me-in"));
        let router = app(&world.auth(loopback(), vec![provider(config, loopback())]));
        assert!(whoami(&router, bearer("let-me-in", LOCAL)).await.is_ok());

        let cases = [
            (Some("let-me-out"), Some("invalid_token")),
            (Some("cairn_agent_00"), Some("invalid_token")),
            (None, None),
        ];
        for (token, error) in cases {
            let request = match token {
                Some(token) => bearer(token, LOCAL),
                None => get("/whoami", LOCAL),
            };
            let response = send(&router, request).await;
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{token:?}");
            let challenge = response.headers()[WWW_AUTHENTICATE].to_str().unwrap();
            assert!(challenge.starts_with("Bearer"), "{challenge}");
            assert_eq!(
                challenge.contains("invalid_token"),
                error.is_some(),
                "{token:?}"
            );
        }
    }

    /// A failing provider never falls through to a weaker one: a wrong token is refused even
    /// though the next provider would sign the request in without one.
    #[tokio::test]
    async fn a_refused_credential_does_not_fall_through_to_another_provider() {
        let world = World::new();
        let providers = vec![
            provider(dev("token", Some("let-me-in")), loopback()),
            provider(dev("ambient", None), loopback()),
        ];
        let router = app(&world.auth(loopback(), providers));
        let token = whoami(&router, bearer("let-me-in", LOCAL)).await.unwrap();
        let ambient = whoami(&router, get("/whoami", LOCAL)).await.unwrap();
        assert_ne!(token.user, ambient.user);
        let wrong = whoami(&router, bearer("let-me-out", LOCAL)).await;
        assert_eq!(wrong, Err(StatusCode::UNAUTHORIZED));
        let basic = request("/whoami", LOCAL).header(AUTHORIZATION, "Basic YTpi");
        let basic = whoami(&router, basic.body(Body::empty()).unwrap()).await;
        assert_eq!(basic, Err(StatusCode::UNAUTHORIZED));
    }

    /// A session cookie signs the browser in until it lapses or is ended; a lapsed, ended,
    /// or forged one is refused and cleared rather than ignored.
    #[tokio::test]
    async fn a_session_signs_in_until_it_lapses_or_ends() {
        let world = World::new();
        let router = app(&world.auth(loopback(), Vec::new()));
        let user = world.accounts.store().put_user(cairn_store::UserRecord {
            id: "u_ann".parse().unwrap(),
            name: "Ann".parse().unwrap(),
            created_at: world.accounts.clock().now(),
        });
        user.await.unwrap();
        let ann = "u_ann".parse().unwrap();
        let token = world.accounts.start_session(&ann).await.unwrap();
        let cookie = format!("cairn_session={}", token.expose());
        let signed_in = || with_cookie(request("/whoami", LOCAL), &cookie).body(Body::empty());
        assert_eq!(
            whoami(&router, signed_in().unwrap()).await.unwrap().user,
            ann
        );

        let forged = with_cookie(request("/whoami", LOCAL), "cairn_session=forged");
        let response = send(&router, forged.body(Body::empty()).unwrap()).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(cookie_header(&cookies_set(&response)), "");
        assert!(!cookies_set(&response).is_empty(), "the cookie is cleared");

        world
            .clock
            .advance(i64::try_from(SESSION_LIFETIME.as_secs()).unwrap());
        assert_eq!(
            whoami(&router, signed_in().unwrap()).await,
            Err(StatusCode::UNAUTHORIZED)
        );

        let renewed = world.accounts.start_session(&ann).await.unwrap();
        let cookie = format!("cairn_session={}", renewed.expose());
        let out = with_cookie(request("/auth/sign-out", LOCAL), &cookie).method("POST");
        let response = send(&router, out.body(Body::empty()).unwrap()).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let after = with_cookie(request("/whoami", LOCAL), &cookie).body(Body::empty());
        assert_eq!(
            whoami(&router, after.unwrap()).await,
            Err(StatusCode::UNAUTHORIZED)
        );
    }
}
