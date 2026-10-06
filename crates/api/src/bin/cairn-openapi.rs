//! Writes the OpenAPI document to a file: `cairn-openapi <path>`. Run by `mise run gen`
//! (mise-tasks/gen/openapi).

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: cairn-openapi <path>");
        return ExitCode::FAILURE;
    };
    if let Err(error) = std::fs::write(&path, cairn_api::openapi::document_text()) {
        eprintln!("cairn-openapi: writing {}: {error}", path.display());
        return ExitCode::FAILURE;
    }
    println!("cairn-openapi: wrote {}", path.display());
    ExitCode::SUCCESS
}
