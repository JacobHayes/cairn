//! ARCHITECTURE's repository layout and crate graph against the workspace they describe: the
//! crates' directories and manifests own both, and the document follows them.
#![cfg(test)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn architecture() -> String {
    std::fs::read_to_string(repository().join("ARCHITECTURE.md")).expect("ARCHITECTURE.md exists")
}

/// A crate's name as the document writes it: without the `cairn-` prefix (the binary's own
/// package is `cairn`).
fn short_name(package: &str) -> &str {
    package.strip_prefix("cairn-").unwrap_or(package)
}

/// The lines of the first code block (fenced with `fence`) that follows `after` in `text`.
fn block<'a>(text: &'a str, after: &str, fence: &str) -> Vec<&'a str> {
    let start = text
        .find(after)
        .unwrap_or_else(|| panic!("no {after:?} in ARCHITECTURE"));
    let rest = &text[start + after.len()..];
    let open = rest.find(fence).expect("a code block follows");
    let body = &rest[open + fence.len()..];
    let body = &body[body.find('\n').expect("the fence line ends") + 1..];
    body[..body.find("```").expect("the block closes")]
        .lines()
        .collect()
}

/// Every crate directory is in the layout, and the layout names no other.
#[test]
fn the_layout_lists_every_crate() {
    let text = architecture();
    let layout = block(&text, "## Repository layout", "```");
    let in_crates = layout
        .iter()
        .skip_while(|line| line.trim() != "crates/")
        .skip(1)
        .take_while(|line| line.starts_with("    "));
    let listed: BTreeSet<String> = in_crates
        .filter_map(|line| line.split_whitespace().next())
        .filter_map(|word| word.strip_suffix('/'))
        .map(str::to_owned)
        .collect();
    let on_disk: BTreeSet<String> = std::fs::read_dir(repository().join("crates"))
        .expect("crates/ exists")
        .map(|entry| entry.expect("a directory entry"))
        .filter(|entry| entry.path().join("Cargo.toml").exists())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(listed, on_disk, "the layout lists the crates there are");
}

/// Each workspace crate's direct normal dependencies on other workspace crates, by short name.
fn workspace_dependencies() -> BTreeMap<String, BTreeSet<String>> {
    let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()))
        .args([
            "metadata",
            "--no-deps",
            "--offline",
            "--format-version",
            "1",
        ])
        .current_dir(repository())
        .output()
        .expect("cargo metadata runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout).expect("metadata is JSON");
    let packages = metadata["packages"].as_array().expect("packages");
    let names: BTreeSet<&str> = packages.iter().filter_map(|p| p["name"].as_str()).collect();
    packages
        .iter()
        .map(|package| {
            let own = package["dependencies"]
                .as_array()
                .expect("dependencies")
                .iter()
                .filter(|dependency| dependency["kind"].is_null())
                .filter_map(|dependency| dependency["name"].as_str())
                .filter(|dependency| names.contains(dependency))
                .map(|dependency| short_name(dependency).to_owned())
                .collect();
            (
                short_name(package["name"].as_str().expect("a name")).to_owned(),
                own,
            )
        })
        .collect()
}

/// Whether `from` depends on `target`, directly or through other crates.
fn reaches(dependencies: &BTreeMap<String, BTreeSet<String>>, from: &str, target: &str) -> bool {
    let mut seen = BTreeSet::new();
    let mut pending = vec![from];
    while let Some(crate_name) = pending.pop() {
        for next in &dependencies[crate_name] {
            if next == target {
                return true;
            }
            if seen.insert(next.as_str()) {
                pending.push(next);
            }
        }
    }
    false
}

/// The crate graph draws each crate's direct dependencies the others do not already imply
/// (a transitive reduction of the manifests' normal dependencies), and no other edge.
#[test]
fn the_crate_graph_is_the_manifests_dependencies() {
    let dependencies = workspace_dependencies();
    let mut expected = BTreeSet::new();
    for (dependent, own) in &dependencies {
        for dependency in own {
            let implied = own
                .iter()
                .any(|other| other != dependency && reaches(&dependencies, other, dependency));
            if !implied {
                expected.insert((dependency.clone(), dependent.clone()));
            }
        }
    }
    let text = architecture();
    let drawn: BTreeSet<(String, String)> = block(&text, "## Repository layout", "```mermaid")
        .iter()
        .filter_map(|line| line.split_once("-->"))
        .map(|(from, to)| (node_id(from), node_id(to)))
        .filter(|(from, to)| dependencies.contains_key(from) && dependencies.contains_key(to))
        .collect();
    assert_eq!(
        drawn, expected,
        "(dependency, dependent) edges of the crate graph"
    );
}

/// A Mermaid node as a crate name: `store_sqlite[store-sqlite]` is `store-sqlite`.
fn node_id(node: &str) -> String {
    let id = node
        .trim()
        .split('[')
        .next()
        .expect("a split has a first part");
    id.replace('_', "-")
}
