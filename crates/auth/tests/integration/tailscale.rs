//! The Tailscale provider (H1), offline: direct mode against a fake tailscaled answering
//! whois on a unix socket, proxy mode through `Tailscale-User-*` headers from this machine or
//! from the proxies it lists, and the refusals that keep each mode where it is safe (3.2,
//! Security).

#[cfg(test)]
mod tailscale {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use axum::Router;
    use axum::body::Body;
    use axum::extract::Query;
    use axum::http::StatusCode;
    use axum::http::request::Builder;
    use axum::response::{IntoResponse, Response};
    use axum::routing::get;
    use cairn_auth::{
        AuthProvider, Listener, TailscaleConfig, TailscaleMode, TailscaleProvider, TrustedProxies,
    };
    use cairn_store::AuthStore;
    use serde_json::json;

    use crate::support::{LOCAL, REMOTE, World, app, loopback, public, request, whoami};

    const ANN_PEER: &str = "100.101.102.103:41641";
    const TAGGED_PEER: &str = "100.101.102.104:41641";
    const STRANGER_PEER: &str = "100.101.102.105:41641";

    fn tailnet() -> Listener {
        Listener::Tcp("100.100.1.1:443".parse().unwrap())
    }

    /// A fake tailscaled on a unix socket, removed when dropped. The socket lives in the
    /// runtime directory when there is one, since cargo's target directory can be too deep
    /// for a socket path.
    struct Tailscaled {
        socket: PathBuf,
    }

    impl Drop for Tailscaled {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.socket);
        }
    }

    fn socket_path() -> PathBuf {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let directory = std::env::var_os("XDG_RUNTIME_DIR")
            .map_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")), PathBuf::from);
        let name = format!(
            "cairn-tailscaled-{}-{}.sock",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = directory.join(name);
        assert!(
            path.as_os_str().len() < 108,
            "socket path too long: {}",
            path.display()
        );
        path
    }

    async fn whois(Query(query): Query<HashMap<String, String>>) -> Response {
        let profile =
            |login: &str, name: &str| json!({"ID": 1, "LoginName": login, "DisplayName": name});
        let body = match query.get("addr").map(String::as_str) {
            Some(ANN_PEER) => {
                json!({"Node": {"ID": 7, "Name": "laptop."}, "UserProfile": profile("ann@example.org", "Ann")})
            }
            Some(TAGGED_PEER) => {
                json!({"Node": {"Tags": ["tag:ci"]}, "UserProfile": profile("tagged-devices", "tagged devices")})
            }
            _ => return (StatusCode::NOT_FOUND, "no match for IP:port").into_response(),
        };
        axum::Json(body).into_response()
    }

    fn tailscaled() -> Tailscaled {
        let socket = socket_path();
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let router = Router::new().route("/localapi/v0/whois", get(whois));
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Tailscaled { socket }
    }

    /// Proxy mode trusting `tailscale serve` on this machine.
    fn this_machine() -> TailscaleMode {
        TailscaleMode::Proxy {
            trusted: TrustedProxies::ThisMachine,
        }
    }

    /// Proxy mode trusting a proxy on another machine at `sources`.
    fn proxies(sources: &[&str]) -> TailscaleMode {
        let sources = sources.iter().map(|source| source.parse().unwrap());
        TailscaleMode::Proxy {
            trusted: TrustedProxies::Sources(sources.collect()),
        }
    }

    /// A listener on a private network address, which a proxy on another machine reaches.
    fn private() -> Listener {
        Listener::Tcp("10.10.10.6:8085".parse().unwrap())
    }

    /// Request headers, by name and value.
    type Headers = [(&'static str, &'static str)];

    /// `builder` carrying each of `headers`.
    fn with_headers(mut builder: Builder, headers: &[(&str, &str)]) -> axum::http::Request<Body> {
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        builder.body(Body::empty()).unwrap()
    }

    fn config(mode: TailscaleMode) -> TailscaleConfig {
        TailscaleConfig {
            name: "tailscale".parse().unwrap(),
            mode,
            auto_link: false,
        }
    }

    fn get_from(peer: &str) -> axum::http::Request<Body> {
        request("/whoami", peer).body(Body::empty()).unwrap()
    }

    /// Direct mode: tailscaled's whois names the peer's user, whose login is a verified
    /// email; a tagged machine signs no one in, a peer tailscaled does not know is refused,
    /// and so is every request when tailscaled cannot be asked. `Tailscale-*` headers are
    /// never read: a forged login changes nothing.
    #[tokio::test]
    async fn direct_mode_asks_tailscaled_who_the_peer_is() {
        let daemon = tailscaled();
        let world = World::new();
        let mode = TailscaleMode::Direct {
            socket: daemon.socket.clone(),
        };
        let provider: Arc<dyn AuthProvider> =
            Arc::new(TailscaleProvider::new(config(mode), tailnet()).unwrap());
        let router = app(&world.auth(tailnet(), vec![provider]));
        let ann = whoami(&router, get_from(ANN_PEER)).await.unwrap();
        let identities = world.store.identities_of(&ann.user).await.unwrap();
        assert_eq!(identities[0].subject.as_str(), "ann@example.org");
        let email = "ann@example.org".parse().unwrap();
        assert!(identities[0].verified_emails.contains(&email));
        assert_eq!(
            whoami(&router, get_from(TAGGED_PEER)).await,
            Err(StatusCode::UNAUTHORIZED)
        );
        assert_eq!(
            whoami(&router, get_from(STRANGER_PEER)).await,
            Err(StatusCode::FORBIDDEN)
        );
        let forged = [("Tailscale-User-Login", "bob@example.org")];
        let forged_by = |peer| with_headers(request("/whoami", peer), &forged);
        let still_ann = whoami(&router, forged_by(ANN_PEER)).await.unwrap();
        assert_eq!(still_ann.user, ann.user);
        let cases = [
            (TAGGED_PEER, StatusCode::UNAUTHORIZED),
            (STRANGER_PEER, StatusCode::FORBIDDEN),
        ];
        for (peer, refused) in cases {
            assert_eq!(
                whoami(&router, forged_by(peer)).await,
                Err(refused),
                "{peer}"
            );
        }

        let gone = TailscaleMode::Direct {
            socket: socket_path(),
        };
        let provider: Arc<dyn AuthProvider> =
            Arc::new(TailscaleProvider::new(config(gone), tailnet()).unwrap());
        let router = app(&world.auth(tailnet(), vec![provider]));
        assert_eq!(
            whoami(&router, get_from(ANN_PEER)).await,
            Err(StatusCode::SERVICE_UNAVAILABLE)
        );
    }

    /// Direct mode serves only a listener bound to a tailnet address; proxy mode trusting
    /// this machine only one the proxy alone reaches, loopback or a unix socket; proxy mode
    /// trusting the proxies it lists any listener, since it checks every peer.
    #[test]
    fn each_mode_starts_only_on_a_listener_where_it_is_safe() {
        let direct = || TailscaleMode::Direct {
            socket: PathBuf::from("/run/tailscale/tailscaled.sock"),
        };
        let cases = [
            (direct(), tailnet(), true),
            (direct(), loopback(), false),
            (direct(), public(), false),
            (direct(), Listener::Unix, false),
            (this_machine(), loopback(), true),
            (this_machine(), Listener::Unix, true),
            (this_machine(), public(), false),
            (this_machine(), tailnet(), false),
            (this_machine(), private(), false),
            (proxies(&["10.10.10.2"]), private(), true),
            (proxies(&["10.10.10.0/24"]), public(), true),
            (proxies(&["10.10.10.2"]), loopback(), true),
            (proxies(&[]), private(), false),
        ];
        for (mode, listener, starts) in cases {
            let started = TailscaleProvider::new(config(mode.clone()), listener);
            assert_eq!(started.is_ok(), starts, "{mode:?} on {listener:?}");
        }
    }

    /// Proxy mode: the proxy's headers name the user, but only from this machine; a request
    /// without them holds nothing of this provider's.
    #[tokio::test]
    async fn proxy_mode_trusts_the_headers_only_from_this_machine() {
        let world = World::new();
        let provider: Arc<dyn AuthProvider> =
            Arc::new(TailscaleProvider::new(config(this_machine()), loopback()).unwrap());
        let router = app(&world.auth(loopback(), vec![provider]));
        let proxied = |peer: &str| {
            request("/whoami", peer)
                .header("Tailscale-User-Login", "ann@example.org")
                .header("Tailscale-User-Name", "Ann")
                .body(Body::empty())
                .unwrap()
        };
        let ann = whoami(&router, proxied(LOCAL)).await.unwrap();
        let user = world.store.user(&ann.user).await.unwrap().unwrap();
        assert_eq!(user.name.as_str(), "Ann");
        assert_eq!(
            whoami(&router, proxied(REMOTE)).await,
            Err(StatusCode::FORBIDDEN)
        );
        assert_eq!(
            whoami(&router, get_from(LOCAL)).await,
            Err(StatusCode::UNAUTHORIZED)
        );
    }

    /// Proxy mode behind a proxy on another machine: the headers name the user only from a
    /// listed source; from anywhere else, loopback included, a request carrying any
    /// `Tailscale-*` header is refused, so forged headers name no one. From the proxy, no
    /// user header is anonymous, and a tagged node signs no one in even beside a login.
    #[tokio::test]
    async fn proxy_mode_trusts_the_headers_only_from_the_proxies_it_lists() {
        const PROXY: &str = "10.10.10.2:52000";
        const NEIGHBOUR: &str = "10.10.10.7:52000";
        let world = World::new();
        let mode = proxies(&["10.10.10.2"]);
        let provider: Arc<dyn AuthProvider> =
            Arc::new(TailscaleProvider::new(config(mode), private()).unwrap());
        let router = app(&world.auth(private(), vec![provider]));
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
                let identities = world.store.identities_of(&actor.user).await.unwrap();
                assert_eq!(identities[0].subject.as_str(), "ann@example.org");
                let user = world.store.user(&actor.user).await.unwrap().unwrap();
                assert_eq!(user.name.as_str(), "Ann");
            }
        }
    }
}
