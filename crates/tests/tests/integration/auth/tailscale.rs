//! The Tailscale provider (H1), offline: `Tailscale-User-*` headers from the proxies it lists,
//! and the refusals that keep forged headers from naming anyone.

#[cfg(test)]
mod tailscale {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::http::request::Builder;
    use cairn_auth::{AuthProvider, TailscaleConfig, TailscaleProvider};
    use cairn_store::AuthStore;

    use crate::auth::support::{LOCAL, REMOTE, World, app, request, whoami};

    /// Request headers, by name and value.
    type Headers = [(&'static str, &'static str)];

    /// `builder` carrying each of `headers`.
    fn with_headers(mut builder: Builder, headers: &[(&str, &str)]) -> axum::http::Request<Body> {
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        builder.body(Body::empty()).unwrap()
    }

    fn config(sources: &[&str]) -> TailscaleConfig {
        TailscaleConfig {
            name: "tailscale".parse().unwrap(),
            trusted_proxies: sources
                .iter()
                .map(|source| source.parse().unwrap())
                .collect(),
        }
    }

    /// The provider starts only with a proxy to trust.
    #[test]
    fn the_provider_needs_a_proxy_to_trust() {
        assert!(TailscaleProvider::new(config(&["10.10.10.2"])).is_ok());
        assert!(TailscaleProvider::new(config(&["10.10.10.0/24"])).is_ok());
        assert!(TailscaleProvider::new(config(&[])).is_err());
    }

    /// A proxy listed as an IPv4-mapped IPv6 address or network is the same proxy.
    #[tokio::test]
    async fn a_mapped_proxy_source_matches_its_ipv4_peer() {
        let world = World::new();
        for source in ["::ffff:10.10.10.2", "::ffff:10.10.10.0/120"] {
            let provider: Arc<dyn AuthProvider> =
                Arc::new(TailscaleProvider::new(config(&[source])).unwrap());
            let router = app(&world.auth(vec![provider]));
            let headers = [("Tailscale-User-Login", "ann@example.org")];
            let request = with_headers(request("/whoami", "10.10.10.2:52000"), &headers);
            assert!(whoami(&router, request).await.is_ok(), "{source}");
        }
    }

    /// The headers name the user only from a listed source; from anywhere else, loopback
    /// included, a request carrying any `Tailscale-*` header is refused, so forged headers
    /// name no one. From the proxy, no user header is anonymous, and a tagged node signs no
    /// one in even beside a login.
    #[tokio::test]
    async fn the_headers_name_the_user_only_from_the_proxies_listed() {
        const PROXY: &str = "10.10.10.2:52000";
        const NEIGHBOUR: &str = "10.10.10.7:52000";
        let world = World::new();
        let provider: Arc<dyn AuthProvider> =
            Arc::new(TailscaleProvider::new(config(&["10.10.10.2"])).unwrap());
        let router = app(&world.auth(vec![provider]));
        let user = [
            ("Tailscale-User-Login", "ann@example.org"),
            ("Tailscale-User-Name", "Ann"),
            ("Tailscale-Node-Name", "laptop.example.ts.net"),
        ];
        let tagged = [
            ("Tailscale-Node-Tags", "tag:ci"),
            ("Tailscale-Node-Name", "ci.example.ts.net"),
        ];
        let tagged_with_login = [
            ("Tailscale-Node-Tags", "tag:ci"),
            ("Tailscale-User-Login", "ann@example.org"),
        ];
        let node_only = [("Tailscale-Node-Name", "laptop.example.ts.net")];
        let unauthorized = Err(StatusCode::UNAUTHORIZED);
        let forbidden = Err(StatusCode::FORBIDDEN);
        let cases: [(&str, &Headers, Result<(), StatusCode>); 10] = [
            (PROXY, &user, Ok(())),
            (PROXY, &[], unauthorized),
            (PROXY, &tagged, unauthorized),
            (PROXY, &tagged_with_login, unauthorized),
            (PROXY, &node_only, unauthorized),
            (NEIGHBOUR, &user, forbidden),
            (NEIGHBOUR, &node_only, forbidden),
            (NEIGHBOUR, &[], unauthorized),
            (LOCAL, &user, forbidden),
            (REMOTE, &tagged, forbidden),
        ];
        for (peer, headers, expected) in cases {
            let answered = whoami(&router, with_headers(request("/whoami", peer), headers)).await;
            let outcome = answered.as_ref().map(|_| ()).map_err(|status| *status);
            assert_eq!(outcome, expected, "{peer} {headers:?}");
            if let Ok(actor) = answered {
                let logins = world.store.logins_of(&actor.user).await.unwrap();
                assert_eq!(logins[0].subject.as_str(), "ann@example.org");
                assert_eq!(
                    logins[0].name.as_ref().map(cairn_schema::Title::as_str),
                    Some("Ann")
                );
            }
        }
    }
}
