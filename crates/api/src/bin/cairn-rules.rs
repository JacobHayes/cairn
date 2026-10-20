//! Writes the rule tables the web app reads to a file: `cairn-rules <path>`. Run by
//! `mise run gen` (mise-tasks/gen/openapi).

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: cairn-rules <path>");
        return ExitCode::FAILURE;
    };
    if let Err(error) = std::fs::write(&path, cairn_engine::rules::typescript()) {
        eprintln!("cairn-rules: writing {}: {error}", path.display());
        return ExitCode::FAILURE;
    }
    println!("cairn-rules: wrote {}", path.display());
    ExitCode::SUCCESS
}
