//! `cairn`: the command line (ARCHITECTURE, Build, run, deploy). `serve` runs the API, MCP,
//! the assistant when configured, SSE, and the embedded UI on one port; `migrate` brings the
//! database's schema up to date; `config check` validates a configuration without starting
//! anything; `version` prints the versions. Every command but `version` reads the
//! configuration file named by `--config` or `CAIRN_CONFIG`, with environment overrides.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use cairn::config::{self, Config, Environment, env::CONFIG_VARIABLE};
use cairn::{root, telemetry};
use cairn_auth::{Accounts, Clock};
use cairn_store::MemoryStore;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "cairn",
    about = "Cairn: processes as graphs of work, for people and agents"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Serve the API, MCP, SSE, the assistant when configured, and the UI on one port.
    Serve(ConfigArgument),
    /// Create the database if absent and apply the migrations it lacks.
    Migrate(ConfigArgument),
    /// Configuration commands.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Print the binary's and the engine's versions.
    Version,
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Validate the configuration, the auth providers included, without opening the
    /// database or listening.
    Check(ConfigArgument),
}

#[derive(clap::Args)]
struct ConfigArgument {
    /// The configuration file (TOML). Defaults to the `CAIRN_CONFIG` variable.
    #[arg(long, value_name = "FILE")]
    config: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let outcome = match cli.command {
        Command::Version => {
            println!(
                "cairn {} (engine {})",
                env!("CARGO_PKG_VERSION"),
                cairn_engine::engine_version().as_str()
            );
            Ok(())
        }
        Command::Config {
            command: ConfigCommand::Check(argument),
        } => load(&argument).and_then(|config| check(&config)),
        Command::Migrate(argument) => load(&argument).and_then(|config| runtime(migrate(config))),
        Command::Serve(argument) => load(&argument).and_then(|config| runtime(serve(config))),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("cairn: {error}");
            ExitCode::FAILURE
        }
    }
}

/// The configuration the argument or `CAIRN_CONFIG` names.
fn load(argument: &ConfigArgument) -> Result<Config, String> {
    let environment = Environment::from_process();
    let path = argument
        .config
        .clone()
        .or_else(|| environment.get(CONFIG_VARIABLE).map(PathBuf::from))
        .ok_or_else(|| format!("no configuration file: pass --config or set {CONFIG_VARIABLE}"))?;
    config::load(&path, &environment).map_err(|problems| problems.to_string())
}

fn runtime(work: impl Future<Output = Result<(), String>>) -> Result<(), String> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("the runtime did not start: {error}"))?
        .block_on(work)
}

/// `config check`: the configuration and its auth providers, checked as `serve` would, over
/// a throwaway store, and summarized without secrets.
fn check(config: &Config) -> Result<(), String> {
    let accounts = Accounts::new(Arc::new(MemoryStore::new()), Clock::system());
    root::providers(config, &accounts)
        .map_err(|problems| root::StartupError::Providers(problems).to_string())?;
    let capabilities = root::capabilities(config);
    let providers: Vec<String> = capabilities
        .auth
        .iter()
        .map(|method| format!("{} ({:?})", method.name, method.kind))
        .collect();
    let hosts = cairn::host::AllowedHosts::new(&config.public_url, config.listen);
    println!("configuration valid");
    println!("database: {}", config.database.display());
    println!("listen: {}", config.listen);
    println!("public url: {}", config.public_url);
    println!(
        "hosts served: {}",
        hosts.names().collect::<Vec<_>>().join(", ")
    );
    println!("time zone: {}", config.timezone.as_str());
    println!("auth providers: {}", providers.join(", "));
    println!(
        "assistant: {}",
        if capabilities.assistant {
            "configured"
        } else {
            "none"
        }
    );
    Ok(())
}

/// `migrate`: opening the store creates the file if absent and applies every migration it
/// lacks, in one transaction.
async fn migrate(config: Config) -> Result<(), String> {
    cairn_store_turso::TursoStore::open(&config.database)
        .await
        .map_err(|error| format!("the database {}: {error}", config.database.display()))?;
    println!("migrated {}", config.database.display());
    Ok(())
}

/// `serve`, until SIGINT or SIGTERM.
async fn serve(config: Config) -> Result<(), String> {
    telemetry::init_logs();
    telemetry::install_panic_policy();
    let metrics = telemetry::install_metrics().map_err(|error| error.to_string())?;
    root::serve(&config, metrics, stopped())
        .await
        .map_err(|error| {
            tracing::error!(%error, "stopped");
            error.to_string()
        })
}

/// Resolves on SIGINT or, on unix, SIGTERM.
async fn stopped() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }
}
