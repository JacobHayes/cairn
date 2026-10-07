//! The binary itself, as a deployment runs it (brief 4.7, Acceptance): a config file and a
//! database file, one port for the UI and the API, startup failures that name what is
//! missing, and a clean stop. `mod commands` needs only the binary (rung 2); `mod binary`
//! needs it built with the web build embedded, so rung 6 runs it after `mise run
//! build:web`.
#![cfg(test)]

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
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
    use crate::support::{self, get};

    /// A started `cairn serve`, stopped (killed) when dropped.
    struct Served {
        child: Child,
        address: std::net::SocketAddr,
    }

    impl Drop for Served {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    /// Starts `cairn serve --config <path>` and waits for its `listening` log line.
    fn serve(path: &Path) -> Served {
        let child = cairn(&["serve", "--config", path.to_str().unwrap()])
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // Held from here, so a test that fails before the server listens still stops it.
        let mut served = Served {
            child,
            address: ([0, 0, 0, 0], 0).into(),
        };
        let stderr = served.child.stderr.take().unwrap();
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
            assert_eq!(log["level"], "INFO", "{line}");
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
        let capabilities = get(address, "localhost", "/capabilities").await.json();
        assert_eq!(capabilities["assistant"], false);
        assert_eq!(capabilities["mcp"], true);
        let document = get(address, "localhost", "/journeys/j_hiring/document").await;
        assert_eq!(document.status, 200, "{}", document.text());
        assert_eq!(
            document.json()["engine_version"],
            cairn_engine::engine_version().as_str()
        );
        let snapshot = get(address, "localhost", "/journeys/j_hiring/snapshot").await;
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
            get(address, "attacker.example", "/capabilities")
                .await
                .status,
            421
        );

        let stopped = Command::new("kill")
            .args(["-TERM", &served.child.id().to_string()])
            .status()
            .unwrap();
        assert!(stopped.success());
        let status = served.child.wait().unwrap();
        assert!(status.success(), "{status}");
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
        let mut child = cairn(&["serve", "--config", path.to_str().unwrap()])
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stderr = BufReader::new(child.stderr.take().unwrap());
        let mut first = String::new();
        stderr.read_line(&mut first).unwrap();
        assert!(first.contains("listening"), "{first}");
        drop(stderr);
        let address = format!("127.0.0.1:{port}").parse().unwrap();
        for _ in 0..3 {
            assert_eq!(get(address, "localhost", "/capabilities").await.status, 200);
        }
        assert!(child.try_wait().unwrap().is_none(), "the server stopped");
        child.kill().unwrap();
        child.wait().unwrap();
    }
}
