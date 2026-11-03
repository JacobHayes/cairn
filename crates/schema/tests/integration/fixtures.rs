//! The shared fixtures (fixtures/): every route file validates against the generated JSON
//! Schema.

#![cfg(test)]

use std::path::{Path, PathBuf};

use cairn_schema::{RouteFile, from_yaml, json_schema, to_json};

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// Every route document the fixtures hold, whether or not a scenario runs over it.
fn route_documents() -> Vec<(String, RouteFile)> {
    let mut directories: Vec<PathBuf> = std::fs::read_dir(fixtures_root())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    directories.sort();
    let mut found = Vec::new();
    for directory in directories {
        let path = directory.join("route.yaml");
        if path.exists() {
            let text = std::fs::read_to_string(&path).unwrap();
            let route =
                from_yaml(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            found.push((path.display().to_string(), route));
        }
    }
    found
}

#[test]
fn every_route_file_validates_against_the_json_schema() {
    let schema: serde_json::Value = serde_json::from_str(&json_schema::route_file()).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    for (name, route) in route_documents() {
        let document: serde_json::Value = serde_json::from_str(&to_json(&route).unwrap()).unwrap();
        let errors: Vec<String> = validator
            .iter_errors(&document)
            .map(|error| error.to_string())
            .collect();
        assert!(errors.is_empty(), "{name}: {errors:#?}");
    }
}
