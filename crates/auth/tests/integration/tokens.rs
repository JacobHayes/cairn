//! Agent tokens (ARCHITECTURE, Auth): minted once, stored as a digest, listed, used as a
//! bearer token naming an agent acting for its user (H2), revoked, and logged.

#[cfg(test)]
mod tokens {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::http::header::AUTHORIZATION;
    use cairn_auth::{AuthError, AuthProvider, OAuthConfig, OAuthServer, Secret};
    use cairn_schema::{Actor, Title};
    use cairn_store::{AuthEvent, AuthStore, MemoryStore, UserRecord};

    use crate::support::{LOCAL, World, app, base, loopback, request, whoami};

    fn oauth(world: &World) -> Arc<dyn AuthProvider> {
        let config = OAuthConfig {
            name: "cairn".parse().unwrap(),
            sign_in_with: None,
        };
        Arc::new(OAuthServer::new(config, world.accounts.clone(), &base()))
    }

    async fn ann(world: &World) -> Actor {
        let user = UserRecord {
            id: "u_ann".parse().unwrap(),
            name: "Ann".parse().unwrap(),
            created_at: world.accounts.clock().now(),
        };
        world.store.put_user(user).await.unwrap();
        Actor {
            user: "u_ann".parse().unwrap(),
            agent: None,
        }
    }

    fn bearer(token: &str) -> axum::http::Request<Body> {
        let builder = request("/whoami", LOCAL).header(AUTHORIZATION, format!("Bearer {token}"));
        builder.body(Body::empty()).unwrap()
    }

    fn named(name: &str) -> Title {
        name.parse().unwrap()
    }

    /// H2: a minted token signs requests in as an agent acting for its user, until it is
    /// revoked; minting and revoking are logged.
    #[tokio::test]
    async fn a_token_is_minted_then_used_then_revoked() {
        let world = World::new();
        let ann = ann(&world).await;
        let router = app(&world.auth(loopback(), vec![oauth(&world)]));
        let minted = world
            .accounts
            .mint_token(&ann, named("nightly"))
            .await
            .unwrap();
        let actor = whoami(&router, bearer(minted.token.expose()))
            .await
            .unwrap();
        assert_eq!(actor.user, ann.user);
        assert_eq!(actor.agent.as_ref(), Some(&minted.agent));

        let listed = world.accounts.tokens_of(&ann.user).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(
            (&listed[0].agent, listed[0].revoked_at),
            (&minted.agent, None)
        );

        world
            .accounts
            .revoke_token(&ann, &minted.agent)
            .await
            .unwrap();
        world
            .accounts
            .revoke_token(&ann, &minted.agent)
            .await
            .unwrap();
        let refused = whoami(&router, bearer(minted.token.expose())).await;
        assert_eq!(refused, Err(StatusCode::UNAUTHORIZED));
        let listed = world.accounts.tokens_of(&ann.user).await.unwrap();
        assert!(listed[0].revoked_at.is_some());
        let logged = [AuthEvent::TokenMinted, AuthEvent::TokenRevoked];
        assert_eq!(world.log().await, logged);
    }

    /// A stored token is never readable in plain text: the store holds its digest, and the
    /// digest does not sign anything in.
    #[tokio::test]
    async fn a_stored_token_is_only_its_digest() {
        let world = World::new();
        let ann = ann(&world).await;
        let router = app(&world.auth(loopback(), vec![oauth(&world)]));
        let minted = world
            .accounts
            .mint_token(&ann, named("script"))
            .await
            .unwrap();
        let stored: &MemoryStore = &world.store;
        let records = stored.agent_tokens_of(&ann.user).await.unwrap();
        let held = records[0].token_hash.as_str();
        assert_ne!(held, minted.token.expose());
        assert!(!minted.token.expose().contains(held));
        assert_eq!(records[0].token_hash, minted.token.digest());
        assert_eq!(
            whoami(&router, bearer(held)).await,
            Err(StatusCode::UNAUTHORIZED)
        );
    }

    /// Only a user manages tokens: an agent cannot mint another agent or revoke a token,
    /// and a user cannot revoke another user's token.
    #[tokio::test]
    async fn only_a_user_mints_or_revokes_their_own_tokens() {
        let world = World::new();
        let ann = ann(&world).await;
        let minted = world.accounts.mint_token(&ann, named("bot")).await.unwrap();
        let agent = Actor {
            agent: Some(minted.agent.clone()),
            ..ann.clone()
        };
        let by_agent = world.accounts.mint_token(&agent, named("child")).await;
        assert!(matches!(by_agent, Err(AuthError::AgentNotAllowed)));
        let revoke = world.accounts.revoke_token(&agent, &minted.agent).await;
        assert_eq!(revoke, Err(AuthError::AgentNotAllowed));
        let bob = Actor {
            user: "u_bob".parse().unwrap(),
            agent: None,
        };
        let revoke = world.accounts.revoke_token(&bob, &minted.agent).await;
        assert_eq!(revoke, Err(AuthError::NoSuchToken));
    }

    /// Cairn's other secrets are not bearer tokens: a session token or a made-up token with
    /// Cairn's prefix is refused.
    #[tokio::test]
    async fn other_cairn_secrets_are_refused_as_bearer_tokens() {
        let world = World::new();
        let ann = ann(&world).await;
        let router = app(&world.auth(loopback(), vec![oauth(&world)]));
        let session = world.accounts.start_session(&ann.user).await.unwrap();
        let unknown = Secret::mint(cairn_auth::secret::SecretKind::Agent).unwrap();
        for token in [session.expose(), unknown.expose(), "cairn_agent_short"] {
            let refused = whoami(&router, bearer(token)).await;
            assert_eq!(refused, Err(StatusCode::UNAUTHORIZED), "{token}");
        }
    }
}
