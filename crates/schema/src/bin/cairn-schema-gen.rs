//! Writes the generated JSON Schema files into a directory: `cairn-schema-gen <dir>`.
//! Run by `mise run gen` (mise-tasks/gen/schema).

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(directory) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: cairn-schema-gen <directory>");
        return ExitCode::FAILURE;
    };
    for (name, content) in cairn_schema::json_schema::generated_files() {
        let path = directory.join(name);
        if let Err(error) = std::fs::write(&path, content) {
            eprintln!("cairn-schema-gen: writing {}: {error}", path.display());
            return ExitCode::FAILURE;
        }
        println!("cairn-schema-gen: wrote {}", path.display());
    }
    ExitCode::SUCCESS
}
