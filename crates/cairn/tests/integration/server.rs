//! The assembled server in process, on loopback through the binary's own listener: the
//! `Host` allowlist in front of everything, the capabilities the configuration offers, the
//! metrics, and the socket-level SSE write stall (brief 4.7, Acceptance). The integration step runs these
//! (`mod in_process`).
#![cfg(test)]

mod in_process {
    use std::sync::Arc;

    use cairn::root;
    use cairn_store::InProcessNotifier;
    use serde_json::json;

    use crate::support::{self, PUBLIC_HOST, get, send};

    /// A request for every surface: the API, its health check, MCP, auth's own routes, the
    /// UI, and metrics.
    const SURFACES: [(&str, &str); 6] = [
        ("GET", "/api/capabilities"),
        ("GET", "/healthz"),
        ("POST", "/api/mcp"),
        ("POST", "/api/auth/sign-out"),
        ("GET", "/"),
        ("GET", "/api/metrics"),
    ];

    const INITIALIZE: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#;

    async fn request(
        address: std::net::SocketAddr,
        host: &str,
        surface: (&str, &str),
        token: Option<&str>,
    ) -> u16 {
        let bearer = token.map(|token| format!("Bearer {token}"));
        let mut headers = vec![
            ("content-type", "application/json"),
            ("accept", "application/json, text/event-stream"),
        ];
        if let Some(bearer) = &bearer {
            headers.push(("authorization", bearer.as_str()));
        }
        let (method, path) = surface;
        let body = (path == "/api/mcp").then_some(INITIALIZE);
        send(address, method, host, path, &headers, body)
            .await
            .status
    }

    /// DNS-rebinding protection
    /// (decisions/2026-10-06-dns-rebinding-protection-is-a-host-allowlist-in-front.md):
    /// a request naming another host is refused on every surface before the auth layer,
    /// with or without a credential; the public host and, on this loopback listener, a
    /// loopback name are served.
    #[tokio::test]
    async fn only_the_configured_hosts_are_served_on_every_surface() {
        let config = support::config(&support::dev_provider(Some("dev-token")));
        let app = root::app(
            &config,
            support::assembly(Arc::new(InProcessNotifier::new())),
        )
        .unwrap();
        let address = support::serve(app).await;
        let loopback = format!("127.0.0.1:{}", address.port());
        for surface in SURFACES {
            for token in [None, Some("dev-token")] {
                for refused in [
                    "attacker.example",
                    "attacker.example:443",
                    "cairn.example.com.attacker.example",
                ] {
                    let status = request(address, refused, surface, token).await;
                    assert_eq!(status, 421, "{surface:?} for {refused} with {token:?}");
                }
            }
            for served in [PUBLIC_HOST, loopback.as_str()] {
                let status = request(address, served, surface, Some("dev-token")).await;
                assert!(
                    (200..300).contains(&status),
                    "{surface:?} for {served}: {status}"
                );
            }
        }
        // Past the allowlist, the auth layer still asks for the credential, except of the
        // health check.
        let anonymous = request(address, PUBLIC_HOST, ("GET", "/api/capabilities"), None).await;
        assert_eq!(anonymous, 401);
        let health = request(address, PUBLIC_HOST, ("GET", "/healthz"), None).await;
        assert_eq!(health, 200);
    }

    /// What a request reached.
    #[derive(Debug, PartialEq, Eq)]
    enum Reached {
        /// The app's page.
        Page,
        /// An endpoint's JSON answer, with this status.
        Endpoint(u16),
        /// The API's answer that nothing is served there.
        NoEndpoint,
        /// Nothing: not found, and not the page.
        Missing,
    }

    /// The route map
    /// (decisions/2026-10-08-the-api-is-served-under-api-and-the-app-owns-the-rest.md): the
    /// server keeps `/api/`, `/.well-known/`, and `/healthz`, and answers every other path
    /// with the app's page, so a screen's address loads; a path the server keeps that no
    /// endpoint serves is no endpoint, never the page.
    #[tokio::test]
    async fn the_server_keeps_its_prefixes_and_every_other_path_is_the_apps_page() {
        let config = support::config(&support::dev_provider(Some("dev-token")));
        let app = root::app(
            &config,
            support::assembly(Arc::new(InProcessNotifier::new())),
        )
        .unwrap();
        let address = support::serve(app).await;
        let page = &support::assets()[0].bytes;
        let headers = [("authorization", "Bearer dev-token")];
        let cases = [
            ("GET", "/", Reached::Page),
            ("GET", "/journeys/j_vendor_eval", Reached::Page),
            (
                "GET",
                "/journeys/j_vendor_eval/nodes/n_plan?hide=action",
                Reached::Page,
            ),
            ("HEAD", "/routes/vendor-evaluation/versions", Reached::Page),
            ("GET", "/capabilities", Reached::Page),
            ("GET", "/healthz", Reached::Endpoint(200)),
            ("GET", "/api/capabilities", Reached::Endpoint(200)),
            ("GET", "/api", Reached::NoEndpoint),
            ("GET", "/api/", Reached::NoEndpoint),
            ("GET", "/api/no-such-endpoint", Reached::NoEndpoint),
            (
                "GET",
                "/api/journeys/j_vendor_eval/no-such-view",
                Reached::NoEndpoint,
            ),
            ("POST", "/api/no-such-endpoint", Reached::NoEndpoint),
            ("GET", "/.well-known/no-such-document", Reached::NoEndpoint),
            ("POST", "/journeys/j_vendor_eval/patches", Reached::Missing),
            ("GET", "/assets/index-gone.js", Reached::Missing),
        ];
        for (method, path, expected) in cases {
            let reply = send(address, method, PUBLIC_HOST, path, &headers, None).await;
            let html = reply
                .head
                .to_ascii_lowercase()
                .contains("content-type: text/html");
            let reached = match reply.status {
                200 if html && (method == "HEAD" || reply.body == page.as_ref()) => Reached::Page,
                404 if reply.body.is_empty() => Reached::Missing,
                404 if reply.json()["error"] == json!("no_such_endpoint") => Reached::NoEndpoint,
                status => Reached::Endpoint(status),
            };
            assert_eq!(reached, expected, "{method} {path}: {}", reply.text());
        }
    }

    /// Behind an authenticating proxy on another machine (Tailscale proxy mode listing the
    /// proxy's address): through the real listener, the proxy's headers sign its user in
    /// at the public host, the same headers from a peer the list does not name (this
    /// machine) are refused, the proxy's anonymous health check is answered, and a request
    /// naming the listener's address rather than the public host is refused before any of
    /// it. The peer is always 127.0.0.1, the one loopback address every kernel answers, so
    /// one server lists it as the proxy and another lists a different machine.
    #[tokio::test]
    async fn behind_a_listed_proxy_only_its_identity_headers_sign_in() {
        async fn serve_listing(proxy: &str) -> std::net::SocketAddr {
            let tailscale = format!(
                "[[auth]]\nkind = \"tailscale\"\nname = \"tailnet\"\nmode = \"proxy\"\ntrusted_proxies = [\"{proxy}\"]\n"
            );
            let mut config = support::config(&tailscale);
            // A listener on every address, as one on a private network is: only the listed
            // proxy's headers are trusted on it, not this machine's.
            config.listen = "0.0.0.0:0".parse().unwrap();
            let app = root::app(
                &config,
                support::assembly(Arc::new(InProcessNotifier::new())),
            )
            .unwrap();
            support::serve(app).await
        }
        let login = [
            ("tailscale-user-login", "ann@example.org"),
            ("tailscale-user-name", "Ann"),
        ];

        let proxied = serve_listing("127.0.0.1").await;
        let me = support::send(proxied, "GET", PUBLIC_HOST, "/api/users/me", &login, None).await;
        assert_eq!(me.status, 200, "{}", me.text());
        let identity = &me.json()["identities"][0];
        assert_eq!(identity["provider"], json!("tailnet"));
        assert_eq!(identity["subject"], json!("ann@example.org"));
        let anonymous =
            support::send(proxied, "GET", PUBLIC_HOST, "/api/users/me", &[], None).await;
        assert_eq!(anonymous.status, 401);
        let health = support::send(proxied, "GET", PUBLIC_HOST, "/healthz", &[], None).await;
        assert_eq!(health.status, 200);
        let listener_address = format!("127.0.0.1:{}", proxied.port());
        let misdirected =
            support::send(proxied, "GET", &listener_address, "/healthz", &[], None).await;
        assert_eq!(misdirected.status, 421);

        let elsewhere = serve_listing("127.0.0.2").await;
        let refused =
            support::send(elsewhere, "GET", PUBLIC_HOST, "/api/users/me", &login, None).await;
        assert_eq!(refused.status, 403, "{}", refused.text());
    }

    /// I5: the assistant is in the capabilities exactly when it is configured; MCP and SSE
    /// always are, with the configured sign-in methods in order.
    #[tokio::test]
    async fn the_capabilities_follow_the_configuration() {
        let assistant = "[assistant]\nprotocol = \"chat_completions\"\nendpoint = \"http://127.0.0.1:9/v1\"\nmodel = \"m\"\n";
        for (extra, offered) in [("", false), (assistant, true)] {
            let config = support::config(&format!("{}{extra}", support::dev_provider(None)));
            let app = root::app(
                &config,
                support::assembly(Arc::new(InProcessNotifier::new())),
            )
            .unwrap();
            let address = support::serve(app).await;
            let capabilities = get(address, PUBLIC_HOST, "/api/capabilities").await.json();
            assert_eq!(capabilities["assistant"], json!(offered));
            assert_eq!(capabilities["mcp"], json!(true));
            assert_eq!(capabilities["sse"], json!(true));
            assert_eq!(
                capabilities["auth"],
                json!([{ "name": "dev", "kind": "dev" }])
            );
        }
    }

    /// The UI is served from the web build: the page at `/` and at its own path, each file
    /// with its media type, bundles cached for good and the page revalidated.
    #[tokio::test]
    async fn the_web_build_is_served_beside_the_api() {
        let config = support::config(&support::dev_provider(None));
        let app = root::app(
            &config,
            support::assembly(Arc::new(InProcessNotifier::new())),
        )
        .unwrap();
        let address = support::serve(app).await;
        for path in ["/", "/index.html"] {
            let page = get(address, PUBLIC_HOST, path).await;
            assert_eq!(page.status, 200);
            assert!(
                page.head.contains("content-type: text/html"),
                "{}",
                page.head
            );
            assert!(
                page.head.contains("cache-control: no-cache"),
                "{}",
                page.head
            );
        }
        let bundle = get(address, PUBLIC_HOST, "/assets/index-test.js").await;
        assert_eq!(bundle.text(), "export {};");
        assert!(bundle.head.contains("immutable"), "{}", bundle.head);
        assert_eq!(
            get(address, PUBLIC_HOST, "/assets/other.js").await.status,
            404
        );
    }

    /// A binary without a web build does not start.
    #[test]
    fn a_server_without_a_web_build_does_not_start() {
        let config = support::config(&support::dev_provider(None));
        let mut assembly = support::assembly(Arc::new(InProcessNotifier::new()));
        assembly
            .assets
            .retain(|asset| asset.path != cairn::assets::INDEX);
        assert!(matches!(
            root::app(&config, assembly),
            Err(root::StartupError::WebBuild(_))
        ));
    }

    /// A provider that refuses this listener stops startup, naming it: the dev provider on
    /// an address other machines reach, without its override.
    #[test]
    fn a_provider_refusing_its_listener_stops_startup() {
        let mut config = support::config(&support::dev_provider(None));
        config.listen = "0.0.0.0:0".parse().unwrap();
        let refused = root::app(
            &config,
            support::assembly(Arc::new(InProcessNotifier::new())),
        );
        let Err(root::StartupError::Providers(problems)) = refused else {
            panic!("the dev provider started off loopback");
        };
        assert_eq!(
            problems
                .iter()
                .map(|problem| problem.key.as_str())
                .collect::<Vec<_>>(),
            ["auth.dev"]
        );
    }

    /// Observability: the request counts and latencies the API records, the derive and
    /// commit durations, all at `/api/metrics`, behind the auth layer.
    #[tokio::test]
    async fn metrics_show_requests_derives_and_commits() {
        static RECORDER: std::sync::OnceLock<metrics_exporter_prometheus::PrometheusHandle> =
            std::sync::OnceLock::new();
        let handle = RECORDER
            .get_or_init(|| {
                metrics_exporter_prometheus::PrometheusBuilder::new()
                    .install_recorder()
                    .unwrap()
            })
            .clone();
        let database = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("metrics-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&database);
        crate::support::seed::seed_fixtures(&database)
            .await
            .unwrap();
        let store = Arc::new(
            cairn_store_turso::TursoStore::open(&database)
                .await
                .unwrap(),
        );
        let config = support::config(&support::dev_provider(Some("dev-token")));
        let assembly = root::Assembly {
            store,
            notifier: Arc::new(InProcessNotifier::new()),
            assets: support::assets(),
            metrics: handle,
            clock: cairn_auth::Clock::system(),
        };
        let address = support::serve(root::app(&config, assembly).unwrap()).await;
        let bearer = [("authorization", "Bearer dev-token")];
        let derived = send(
            address,
            "GET",
            PUBLIC_HOST,
            "/api/journeys/j_hiring/derived",
            &bearer,
            None,
        )
        .await;
        assert_eq!(derived.status, 200, "{}", derived.text());
        assert_eq!(get(address, PUBLIC_HOST, "/api/metrics").await.status, 401);
        let metrics = send(address, "GET", PUBLIC_HOST, "/api/metrics", &bearer, None)
            .await
            .text();
        for name in [
            cairn_api::observe::REQUESTS,
            cairn_api::observe::REQUEST_DURATION,
            cairn_service::DERIVE_DURATION,
            cairn_store_turso::COMMIT_DURATION,
        ] {
            assert!(metrics.contains(name), "{name} missing from:\n{metrics}");
        }
    }

    /// decisions/2026-10-06-where-the-sse-write-stall-is-enforced.md: an SSE client that
    /// stops reading, with the stream still ticking, loses its subscriber slot within the
    /// connection's user timeout plus one coalescing interval of its receive window closing.
    /// The kernel closes the connection at `TCP_USER_TIMEOUT`, long before the server's send
    /// buffer could fill and stall the API's own channel. The kernel starts that clock at its
    /// first zero-window probe, one retransmission timeout (at least Linux's 200 ms minimum)
    /// after the window closes, which the bound allows for. The listener is given a one-second
    /// user timeout rather than the write stall the binary sets (`listener::tests` checks that
    /// one), so the test waits a second, not the stall
    /// (decisions/2026-10-08-the-write-stall-test-runs-at-a-one-second-user-timeout.md); the
    /// API's own stall cannot free the slot that soon, so the socket option is what freed it.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_subscriber_that_stops_reading_loses_its_slot_at_the_user_timeout() {
        use std::io::{Read, Write};
        use std::time::{Duration, Instant};

        use cairn::limits::{SSE_COALESCING_INTERVAL, SSE_WRITE_STALL};
        use cairn_schema::{Domain, Revision, RevisionOf};
        use cairn_store::Notifier;

        /// The revision stream of every journey.
        const JOURNEY_TICKS: &str = "/api/events/stream?domain=journeys";

        let user_timeout = Duration::from_secs(1);
        let notifier = Arc::new(InProcessNotifier::new());
        let config = support::config(&support::dev_provider(None));
        let app = root::app(&config, support::assembly(Arc::clone(&notifier))).unwrap();
        let address = support::serve_with_user_timeout(app, user_timeout).await;

        // A subscriber with the smallest receive buffer the kernel allows, so its window
        // closes within a moment of ticks once it stops reading.
        let socket =
            socket2::Socket::new(socket2::Domain::IPV4, socket2::Type::STREAM, None).unwrap();
        socket.set_recv_buffer_size(1).unwrap();
        socket.connect(&address.into()).unwrap();
        let mut client: std::net::TcpStream = socket.into();
        let request = format!("GET {JOURNEY_TICKS} HTTP/1.1\r\nhost: {PUBLIC_HOST}\r\n\r\n");
        client.write_all(request.as_bytes()).unwrap();
        let mut head = [0_u8; 64];
        let read = client.read(&mut head).unwrap();
        assert!(
            head[..read].starts_with(b"HTTP/1.1 200"),
            "{}",
            String::from_utf8_lossy(&head[..read])
        );
        assert_eq!(notifier.subscriber_count(), 1);

        // It reads no more. The stream keeps ticking: a hundred journeys move four times a
        // coalescing interval.
        let ticking = Arc::clone(&notifier);
        let ticker = tokio::spawn(async move {
            let mut revision = Revision::NONE;
            loop {
                revision = revision.next();
                for number in 0..100 {
                    let journey = format!("j_tick_{number}").parse().unwrap();
                    ticking.publish(&RevisionOf::Domain(Domain::Journey(journey)), revision);
                }
                tokio::time::sleep(SSE_COALESCING_INTERVAL / 4).await;
            }
        });

        // Its receive window has closed once the bytes waiting in it stop growing.
        client.set_nonblocking(true).unwrap();
        let mut waiting = 0;
        let mut window_closed = Instant::now();
        let mut buffer = vec![0_u8; 1 << 20];
        let started = Instant::now();
        while notifier.subscriber_count() > 0 {
            assert!(
                started.elapsed() < SSE_WRITE_STALL,
                "the slot was not freed at the socket"
            );
            let now_waiting = client.peek(&mut buffer).unwrap_or(waiting);
            if now_waiting > waiting {
                waiting = now_waiting;
                window_closed = Instant::now();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let freed = window_closed.elapsed();
        ticker.abort();
        eprintln!(
            "freed {freed:?} after the window closed, {:?} after reading stopped",
            started.elapsed()
        );
        assert!(
            freed >= user_timeout,
            "freed after {freed:?}, before the user timeout"
        );
        // Linux's minimum retransmission timeout (TCP_RTO_MIN): the delay before the first
        // zero-window probe, from which the user timeout counts.
        let first_probe = Duration::from_millis(200);
        assert!(
            freed <= user_timeout + SSE_COALESCING_INTERVAL + first_probe,
            "freed after {freed:?}, past the user timeout and one interval"
        );
    }

    /// PRACTICES, Explicit limits: requests in flight are bounded per process, so the UI and
    /// `/api/metrics` share the API's limit. With the API full of requests whose bodies have not
    /// arrived, every surface answers 503 until they go.
    #[tokio::test]
    async fn every_surface_shares_the_in_flight_limit() {
        use std::time::{Duration, Instant};

        use cairn_api::limits::REQUEST_IN_FLIGHT_COUNT_MAX;
        use tokio::io::AsyncWriteExt;

        let config = support::config(&support::dev_provider(None));
        let app = root::app(
            &config,
            support::assembly(Arc::new(InProcessNotifier::new())),
        )
        .unwrap();
        let address = support::serve(app).await;
        let mut held = Vec::new();
        for _ in 0..REQUEST_IN_FLIGHT_COUNT_MAX {
            let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
            let request = format!(
                "POST /api/deployment/patches HTTP/1.1\r\nhost: {PUBLIC_HOST}\r\ncontent-type: application/json\r\ncontent-length: 64\r\n\r\n{{"
            );
            stream.write_all(request.as_bytes()).await.unwrap();
            held.push(stream);
        }
        let started = Instant::now();
        while get(address, PUBLIC_HOST, "/api/capabilities").await.status != 503 {
            assert!(
                started.elapsed() < Duration::from_secs(3),
                "the API never filled"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        for path in ["/api/metrics", "/", "/assets/index-test.js"] {
            assert_eq!(get(address, PUBLIC_HOST, path).await.status, 503, "{path}");
        }
        drop(held);
        let started = Instant::now();
        while get(address, PUBLIC_HOST, "/api/metrics").await.status != 200 {
            assert!(
                started.elapsed() < Duration::from_secs(3),
                "the slots were never freed"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}
