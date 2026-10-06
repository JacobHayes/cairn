//! Checks a document and writes it in canonical form: `cairn-document <type> <file>`, where
//! type is `route-file`, `scenario`, `patch`, or `graph`. Reads YAML (`.yaml`, `.yml`) or
//! JSON; on success prints the canonical YAML and exits 0, on failure prints the error with
//! the path to the failing value and exits 1. For route authors checking a file before
//! import, and for the brief proofs.

use std::process::ExitCode;

use cairn_schema::{
    Graph, ParseError, Patch, RouteFile, Scenario, WriteError, from_json, from_yaml, to_yaml,
};

fn canonical<T: serde::Serialize + serde::de::DeserializeOwned>(
    text: &str,
    yaml: bool,
) -> Result<String, String> {
    let value: T = if yaml {
        from_yaml(text)
    } else {
        from_json(text)
    }
    .map_err(|error: ParseError| error.to_string())?;
    to_yaml(&value).map_err(|error: WriteError| error.to_string())
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [kind, path] = arguments.as_slice() else {
        eprintln!("usage: cairn-document <route-file|scenario|patch|graph> <file>");
        return ExitCode::from(2);
    };
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("cairn-document: reading {path}: {error}");
            return ExitCode::from(2);
        }
    };
    let json = std::path::Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"));
    let yaml = !json;
    let result = match kind.as_str() {
        "route-file" => canonical::<RouteFile>(&text, yaml),
        "scenario" => canonical::<Scenario>(&text, yaml),
        "patch" => canonical::<Patch>(&text, yaml),
        "graph" => canonical::<Graph>(&text, yaml),
        other => {
            eprintln!("cairn-document: unknown document type {other:?}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(canonical) => {
            print!("{canonical}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            println!("{error}");
            ExitCode::FAILURE
        }
    }
}
