//! Writes the agreement cases for the browser tests: every group the server walk
//! gives, a generated 2,000-node journey, and the document at the limits, one JSON file each,
//! with an index, into the directory named first. `scripts/agreement-cases` runs it.

use std::path::Path;

use cairn_wasm::cases::{Group, budget_group, limits_group, server_groups};

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

fn main() {
    let Some(out) = std::env::args().nth(1) else {
        panic!("usage: cairn-wasm-cases DIR");
    };
    let out = Path::new(&out);
    std::fs::create_dir_all(out).unwrap_or_else(|error| panic!("{}: {error}", out.display()));
    let mut groups: Vec<Group> = server_groups();
    groups.push(budget_group(2_000));
    groups.push(limits_group());
    let mut index = Vec::new();
    for (number, group) in groups.iter().enumerate() {
        let file = format!("group-{number:02}.json");
        write(
            &out.join(&file),
            &serde_json::to_string(group).unwrap_or_default(),
        );
        index.push(
            serde_json::json!({ "label": group.label, "file": file, "cases": group.cases.len() }),
        );
    }
    write(
        &out.join("index.json"),
        &serde_json::Value::from(index).to_string(),
    );
    println!(
        "cases: {} groups written to {}",
        groups.len(),
        out.display()
    );
}
