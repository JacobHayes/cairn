//! The binary itself, as a deployment runs it (brief 4.7, Acceptance): a config file and a
//! database file, one port for the UI and the API, startup failures that name what is
//! missing, a clean stop, and a patch it acknowledged still in the file after it. `mod
//! commands` needs only the binary (rung 2); `mod binary` needs it built with the web build
//! embedded, so rung 6 runs it after `mise run build:web`.
#![cfg(test)]

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, Command, ExitStatus, Output, Stdio};
use std::sync::mpsc;

/// The binary cargo built for these tests.
const CAIRN: &str = env!("CARGO_BIN_EXE_cairn");

/// A fresh directory for one test's files.
fn directory(name: &str) -> PathBuf {
    let directory =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("cairn-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

/// Writes `text` as `cairn.toml` in `directory`, answering its path.
fn config_file(directory: &Path, text: &str) -> PathBuf {
    let path = directory.join("cairn.toml");
    std::fs::write(&path, text).unwrap();
    path
}

/// A deployment on a free loopback port, in UTC, with the dev provider.
const CONFIG: &str = r#"
database = "cairn.db"
listen = "127.0.0.1:0"
public_url = "http://localhost/"
timezone = "UTC"

[[auth]]
kind = "dev"
name = "dev"
user = "Dev"
"#;

/// `cairn <arguments>`, with none of the caller's `CAIRN_` variables: a developer's
/// overrides or secrets must not reach the binary a test runs.
fn cairn(arguments: &[&str]) -> Command {
    let mut command = Command::new(CAIRN);
    command.args(arguments);
    for (name, _) in
        std::env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("CAIRN_"))
    {
        command.env_remove(name);
    }
    command
}

/// Runs `cairn <arguments>` to its end, with no `CAIRN_` variables but `environment`.
fn run(arguments: &[&str], environment: &[(&str, &str)]) -> Output {
    cairn(arguments)
        .envs(environment.iter().copied())
        .output()
        .unwrap()
}

mod commands {
    use super::*;

    /// A9 and ARCHITECTURE, Build, run, deploy: a deployment value with no default fails
    /// startup, naming each one missing, before anything is opened.
    #[test]
    fn a_missing_time_zone_or_public_url_fails_startup_naming_it() {
        let directory = directory("missing");
        let text = CONFIG
            .lines()
            .filter(|line| !line.starts_with("timezone") && !line.starts_with("public_url"))
            .collect::<Vec<_>>()
            .join("\n");
        let path = config_file(&directory, &text);
        for command in [&["serve"][..], &["migrate"], &["config", "check"]] {
            let arguments = [command, &["--config", path.to_str().unwrap()]].concat();
            let output = run(&arguments, &[]);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "{command:?} started");
            for key in ["timezone:", "public_url:"] {
                assert!(
                    stderr.contains(key),
                    "{command:?} did not name {key}: {stderr}"
                );
            }
        }
        assert!(
            !directory.join("cairn.db").exists(),
            "a database was opened"
        );
    }

    /// A database that does not open stops `serve` and `migrate` with a nonzero exit
    /// naming it; the process never retries the open, which Turso cannot survive in one
    /// process
    /// (decisions/2026-10-07-a-database-that-does-not-open-stops-the-binary-it-never.md).
    #[test]
    fn a_database_that_does_not_open_stops_the_process() {
        let directory = directory("unopenable");
        let text = CONFIG.replace("cairn.db", "missing-directory/cairn.db");
        let path = config_file(&directory, &text);
        for command in ["serve", "migrate"] {
            let output = run(&[command, "--config", path.to_str().unwrap()], &[]);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "{command} went on");
            assert!(stderr.contains("missing-directory"), "{command}: {stderr}");
        }
    }

    /// The environment supplies what the file leaves out, and `CAIRN_CONFIG` names the file.
    #[test]
    fn the_environment_completes_the_file() {
        let directory = directory("environment");
        let text = CONFIG.replace("timezone = \"UTC\"\n", "");
        let path = config_file(&directory, &text);
        let checked = run(
            &["config", "check"],
            &[
                ("CAIRN_CONFIG", path.to_str().unwrap()),
                ("CAIRN_TIMEZONE", "Pacific/Auckland"),
            ],
        );
        let stdout = String::from_utf8_lossy(&checked.stdout);
        assert!(
            checked.status.success(),
            "{}",
            String::from_utf8_lossy(&checked.stderr)
        );
        assert!(stdout.contains("Pacific/Auckland"), "{stdout}");
        assert!(
            run(&["config", "check"], &[])
                .stderr
                .starts_with(b"cairn: no configuration")
        );
    }

    /// `migrate` creates the database file and is idempotent; `version` names the engine's.
    #[test]
    fn migrate_creates_the_database_and_version_names_the_engine() {
        let directory = directory("migrate");
        let path = config_file(&directory, CONFIG);
        for _ in 0..2 {
            let migrated = run(&["migrate", "--config", path.to_str().unwrap()], &[]);
            assert!(
                migrated.status.success(),
                "{}",
                String::from_utf8_lossy(&migrated.stderr)
            );
        }
        assert!(directory.join("cairn.db").exists());
        let version = String::from_utf8(run(&["version"], &[]).stdout).unwrap();
        assert!(
            version.contains(cairn_engine::engine_version().as_str()),
            "{version}"
        );
    }
}

mod binary {
    use super::*;
    use crate::support::{self, get, send};
    use cairn_store::{Document, LoadTarget, Store};
    use cairn_store_turso::TursoStore;
    use serde_json::json;

    /// A started `cairn serve`, stopped (killed) when dropped unless a test has already
    /// waited for it.
    struct Served {
        child: Option<Child>,
        address: std::net::SocketAddr,
    }

    impl Served {
        /// Starts `cairn serve --config <path>` and keeps a cleanup guard from then on.
        fn spawn(path: &Path) -> Self {
            let child = cairn(&["serve", "--config", path.to_str().unwrap()])
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            Self {
                child: Some(child),
                address: ([0, 0, 0, 0], 0).into(),
            }
        }

        fn child(&mut self) -> &mut Child {
            self.child.as_mut().unwrap()
        }

        fn stderr(&mut self) -> ChildStderr {
            self.child().stderr.take().unwrap()
        }

        fn pid(&self) -> String {
            self.child.as_ref().unwrap().id().to_string()
        }

        fn terminate(&mut self) -> ExitStatus {
            let stopped = Command::new("kill")
                .args(["-TERM", &self.pid()])
                .status()
                .unwrap();
            assert!(stopped.success());
            self.wait()
        }

        fn wait(&mut self) -> ExitStatus {
            self.child.take().unwrap().wait().unwrap()
        }

        fn is_running(&mut self) -> bool {
            self.child().try_wait().unwrap().is_none()
        }
    }

    impl Drop for Served {
        fn drop(&mut self) {
            let Some(mut child) = self.child.take() else {
                return;
            };
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Starts `cairn serve --config <path>` and waits for its `listening` log line.
    fn serve(path: &Path) -> Served {
        // Held from here, so a test that fails before the server listens still stops it.
        let mut served = Served::spawn(path);
        let stderr = served.stderr();
        let (lines, received) = mpsc::channel();
        std::thread::spawn(move || {
            // Drains the log to its end, so the server never writes to a closed pipe; lines
            // after `listening` are echoed for a failing test's output.
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if let Err(unsent) = lines.send(line) {
                    eprintln!("{}", unsent.0);
                }
            }
        });
        loop {
            let line = received
                .recv_timeout(std::time::Duration::from_secs(60))
                .expect("cairn serve logged no `listening` line");
            let Ok(log) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            if log["fields"]["message"] == "listening" {
                served.address = log["fields"]["address"].as_str().unwrap().parse().unwrap();
                return served;
            }
        }
    }

    /// The acceptance smoke test: a fresh directory holding a config file and a database
    /// with the fixtures serves the UI and the API on one port; a fixture journey's
    /// document and snapshot come back; SIGTERM stops it cleanly.
    #[tokio::test]
    async fn the_binary_serves_ui_and_api_on_one_port_and_stops() {
        let directory = directory("smoke");
        let path = config_file(&directory, CONFIG);
        support::seed::seed_fixtures(&directory.join("cairn.db"))
            .await
            .unwrap();
        let mut served = serve(&path);
        let address = served.address;
        let capabilities = get(address, "localhost", "/api/capabilities").await.json();
        assert_eq!(capabilities["assistant"], false);
        assert_eq!(capabilities["mcp"], true);
        let document = get(address, "localhost", "/api/journeys/j_hiring/document").await;
        assert_eq!(document.status, 200, "{}", document.text());
        assert_eq!(
            document.json()["engine_version"],
            cairn_engine::engine_version().as_str()
        );
        let snapshot = get(address, "localhost", "/api/journeys/j_hiring/snapshot").await;
        assert_eq!(snapshot.status, 200, "{}", snapshot.text());
        assert!(
            snapshot.json()["value"]["counts"].is_object(),
            "{}",
            snapshot.text()
        );
        let page = get(address, "localhost", "/").await;
        assert_eq!(page.status, 200);
        assert!(page.text().contains("<script"), "{}", page.text());
        assert_eq!(
            get(address, "attacker.example", "/api/capabilities")
                .await
                .status,
            421
        );

        let status = served.terminate();
        assert!(status.success(), "{status}");
    }

    /// A17, J2: a patch the running binary acknowledges is in its database file. Over HTTP a
    /// journey is created and given a node, the binary is stopped, and the file, opened
    /// again by the store, holds the journey at the revision the binary answered, the node,
    /// and both patches' receipts.
    #[tokio::test]
    async fn an_acknowledged_patch_is_in_the_database_file_after_a_stop() {
        let directory = directory("persisted");
        let path = config_file(&directory, CONFIG);
        let mut served = serve(&path);
        let patches = [
            json!({ "id": "p_create", "target": { "journey": "j_persisted" }, "base_revision": 0,
                "mutations": [{ "op": "create_journey", "name": "Persisted" }] }),
            json!({ "id": "p_node", "target": { "journey": "j_persisted" }, "base_revision": 1,
                "mutations": [{ "op": "add_node", "node": { "key": "n_kept", "id": "kept",
                    "kind": "action", "title": "Kept" } }] }),
        ];
        let mut revision = 0;
        for patch in &patches {
            let body = json!({ "patch": patch }).to_string();
            let reply = send(
                served.address,
                "POST",
                "localhost",
                "/api/journeys/j_persisted/patches",
                &[("content-type", "application/json")],
                Some(&body),
            )
            .await;
            assert_eq!(reply.status, 200, "{}", reply.text());
            revision = reply.json()["receipt"]["revision"].as_u64().unwrap();
        }

        assert!(served.terminate().success());

        let store = TursoStore::open(&directory.join("cairn.db")).await.unwrap();
        let target = LoadTarget::Journey("j_persisted".parse().unwrap());
        let Some(Document::Journey(journey)) = store.load(&target).await.unwrap() else {
            panic!("the journey is not in the database file");
        };
        assert_eq!(u64::from(journey.revision.get()), revision);
        let node = "n_kept".parse().unwrap();
        assert!(journey.graph.nodes.as_map().contains_key(&node));
        for patch in ["p_create", "p_node"] {
            let receipt = store.receipt(&patch.parse().unwrap()).await.unwrap();
            assert!(receipt.is_some(), "{patch} has no receipt");
        }
    }

    /// PRACTICES, Errors, panics, and rejections: a log line that cannot be written (its
    /// reader gone, as under a supervisor that restarted) is lost, never a panic that
    /// would stop the process.
    #[tokio::test]
    async fn a_closed_log_stream_does_not_stop_the_server() {
        let directory = directory("closed-log");
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let text = CONFIG.replace("127.0.0.1:0", &format!("127.0.0.1:{port}"));
        let path = config_file(&directory, &text);
        let mut served = Served::spawn(&path);
        let mut stderr = BufReader::new(served.stderr());
        let mut line = String::new();
        loop {
            line.clear();
            stderr.read_line(&mut line).unwrap();
            assert!(!line.is_empty(), "cairn serve logged no `listening` line");
            if line.contains("listening") {
                break;
            }
        }
        drop(stderr);
        let address = format!("127.0.0.1:{port}").parse().unwrap();
        for _ in 0..3 {
            assert_eq!(
                get(address, "localhost", "/api/capabilities").await.status,
                200
            );
        }
        assert!(served.is_running(), "the server stopped");
        assert!(served.terminate().success());
    }
}
