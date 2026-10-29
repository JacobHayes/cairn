//! The auth layer on a bare axum router (readiness ruling: 4.2's API does not exist yet),
//! with the dev provider and browser sessions: H1 (a simple local mode), and the security
//! rules a request meets whatever provider it reaches (3.2, Security).

#[cfg(test)]
mod layer {
    use std::collections::BTreeSet;
    use std::net::SocketAddr;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
    use cairn_auth::lifetimes::SESSION_LIFETIME;
    use cairn_auth::{AuthProvider, DevConfig, DevProvider};
    use cairn_schema::UserId;
    use cairn_store::{AuthEvent, AuthStore};

    use crate::auth::support::{
        LOCAL, REMOTE, World, app, cookie_header, cookies_set, loopback, public, request, send,
        whoami, with_cookie,
    };

    fn dev(name: &str, token: Option<&str>) -> DevConfig {
        DevConfig {
            name: name.parse().unwrap(),
            user: format!("{name} user").parse().unwrap(),
            verified_emails: BTreeSet::from(["dev@example.org".parse().unwrap()]),
            token: token.map(str::to_owned),
            allow_off_loopback: false,
        }
    }

    /// A user with one login verified for `email`.
    async fn signed_up(world: &World, user: &str, email: &str) -> UserId {
        let user: UserId = user.parse().unwrap();
        let store = world.accounts.store();
        let created_at = world.accounts.clock().now();
        let record = cairn_store::UserRecord {
            id: user.clone(),
            created_at,
        };
        store.put_user(record).await.unwrap();
        store
            .put_login(cairn_store::LoginRecord {
                provider: "oidc".parse().unwrap(),
                subject: user.as_str().parse().unwrap(),
                user: user.clone(),
                name: None,
                verified_emails: [email.parse().unwrap()].into(),
                linked_at: created_at,
            })
            .await
            .unwrap();
        user
    }

    fn provider(config: DevConfig, listen: SocketAddr) -> Arc<dyn AuthProvider> {
        Arc::new(DevProvider::new(config, listen).unwrap())
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
        let auth = world.auth(vec![provider(dev("dev", None), loopback())]);
        let router = app(&auth);
        let first = whoami(&router, get("/whoami", LOCAL)).await.unwrap();
        let again = whoami(&router, get("/whoami", LOCAL)).await.unwrap();
        assert_eq!(first, again);
        assert_eq!(first.agent, None);
        let logins = world.store.logins_of(&first.user).await.unwrap();
        assert_eq!(
            logins[0].name.as_ref().map(cairn_schema::Title::as_str),
            Some("dev user")
        );
        let logged = [AuthEvent::UserCreated, AuthEvent::LoginLinked];
        assert_eq!(world.log().await, logged);
    }

    /// Dev mode is refused off loopback without the override: it does not start on an
    /// address other machines reach, and refuses a request from another machine.
    #[tokio::test]
    async fn dev_mode_is_refused_off_loopback_without_the_override() {
        let refused = DevProvider::new(dev("dev", None), public());
        assert!(refused.is_err());
        let overridden = DevConfig {
            allow_off_loopback: true,
            ..dev("dev", None)
        };
        let world = World::new();
        let open = world.auth(vec![provider(overridden, public())]);
        assert!(whoami(&app(&open), get("/whoami", REMOTE)).await.is_ok());

        let local = world.auth(vec![provider(dev("dev", None), loopback())]);
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
        let router = app(&world.auth(vec![provider(config, loopback())]));
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
        let router = app(&world.auth(providers));
        let token = whoami(&router, bearer("let-me-in", LOCAL)).await.unwrap();
        let ambient = whoami(&router, get("/whoami", LOCAL)).await.unwrap();
        assert_ne!(token.user, ambient.user);
        let wrong = whoami(&router, bearer("let-me-out", LOCAL)).await;
        assert_eq!(wrong, Err(StatusCode::UNAUTHORIZED));
        let basic = request("/whoami", LOCAL).header(AUTHORIZATION, "Basic YTpi");
        let basic = whoami(&router, basic.body(Body::empty()).unwrap()).await;
        assert_eq!(basic, Err(StatusCode::UNAUTHORIZED));
    }

    /// A blank `Authorization`, as IAP passes on once it has taken a Google ID token from
    /// it, carries nothing: the next provider signs the request in as if it were absent. A
    /// wrong token beside a blank value is still refused.
    #[tokio::test]
    async fn a_blank_authorization_counts_as_absent() {
        let world = World::new();
        let providers = vec![
            provider(dev("token", Some("let-me-in")), loopback()),
            provider(dev("ambient", None), loopback()),
        ];
        let router = app(&world.auth(providers));
        let ambient = whoami(&router, get("/whoami", LOCAL)).await.unwrap();
        for blank in ["", "  "] {
            let request = request("/whoami", LOCAL).header(AUTHORIZATION, blank);
            let actor = whoami(&router, request.body(Body::empty()).unwrap()).await;
            assert_eq!(actor.unwrap().user, ambient.user, "{blank:?}");
        }
        let beside = request("/whoami", LOCAL)
            .header(AUTHORIZATION, "")
            .header(AUTHORIZATION, "Bearer let-me-out");
        let beside = whoami(&router, beside.body(Body::empty()).unwrap()).await;
        assert_eq!(beside, Err(StatusCode::UNAUTHORIZED));
    }

    /// A session cookie signs the browser in until it lapses or is ended; a lapsed, ended,
    /// or forged one signs no one in, and the refusal clears it.
    #[tokio::test]
    async fn a_session_signs_in_until_it_lapses_or_ends() {
        let world = World::new();
        let router = app(&world.auth(Vec::new()));
        let ann = signed_up(&world, "u_ann", "ann@example.org").await;
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
        let out = with_cookie(request("/api/auth/sign-out", LOCAL), &cookie).method("POST");
        let response = send(&router, out.body(Body::empty()).unwrap()).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let after = with_cookie(request("/whoami", LOCAL), &cookie).body(Body::empty());
        assert_eq!(
            whoami(&router, after.unwrap()).await,
            Err(StatusCode::UNAUTHORIZED)
        );
    }
}
