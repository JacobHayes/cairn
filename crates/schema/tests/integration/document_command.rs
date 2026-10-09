//! `cairn-document`, the command route authors check a file with: it prints a document of
//! the type it is told in canonical YAML and exits 0, prints the parser's error with the
//! path to the failing value and exits 1, and exits 2 on a request it cannot carry out.
//! YAML or JSON is chosen by the file's extension.
#![cfg(test)]

use std::path::{Path, PathBuf};
use std::process::Output;

use cairn_schema::{Graph, ParseError, Patch, RouteFile, Scenario, from_yaml, to_json};

/// The command cargo built for these tests.
const COMMAND: &str = env!("CARGO_BIN_EXE_cairn-document");

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// A fresh directory for one test's files.
fn directory(name: &str) -> PathBuf {
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("cairn-document-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn run(arguments: &[&str]) -> Output {
    std::process::Command::new(COMMAND)
        .args(arguments)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

/// Runs the command on `path` as `kind`, which must succeed, and answers what it printed.
fn canonical(kind: &str, path: &Path) -> String {
    let output = run(&[kind, path.to_str().unwrap()]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{kind} {}: {}{}",
        path.display(),
        stdout(&output),
        String::from_utf8_lossy(&output.stderr)
    );
    stdout(&output)
}

/// Every fixture is in canonical form, so the command prints each one back byte for byte,
/// read as the type it is.
#[test]
fn every_fixture_prints_back_unchanged() {
    let mut checked = 0;
    for entry in std::fs::read_dir(fixtures_root()).unwrap() {
        let directory = entry.unwrap().path();
        if !directory.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(&directory).unwrap() {
            let path = file.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let kind = if name.starts_with("route") || name.starts_with("segment") {
                "route-file"
            } else {
                "scenario"
            };
            let text = std::fs::read_to_string(&path).unwrap();
            assert_eq!(canonical(kind, &path), text, "{}", path.display());
            checked += 1;
        }
    }
    assert_ne!(checked, 0, "no fixture was found");
}

/// A `.json` file, in any case, is read as JSON; anything else as YAML. A fixture written as
/// JSON comes back as the same canonical YAML under either name (JSON is YAML), so what tells
/// them apart is a YAML document under a JSON name.
#[test]
fn the_extension_chooses_json_or_yaml() {
    let directory = directory("extension");
    let path = fixtures_root().join("vendor-evaluation/route.yaml");
    let text = std::fs::read_to_string(&path).unwrap();
    let file: RouteFile = from_yaml(&text).unwrap();
    let json = to_json(&file).unwrap();
    for name in ["route.json", "route.JSON", "route.yml"] {
        let written = directory.join(name);
        std::fs::write(&written, &json).unwrap();
        assert_eq!(canonical("route-file", &written), text, "{name}");
    }
    // A YAML document under a JSON name, in any case, is read as JSON, so it fails.
    for name in ["misnamed.json", "misnamed.Json"] {
        let misnamed = directory.join(name);
        std::fs::write(&misnamed, &text).unwrap();
        let output = run(&["route-file", misnamed.to_str().unwrap()]);
        assert_eq!(output.status.code(), Some(1), "{name}");
    }
}

/// Each document type is read as that type: a patch and a graph print back as themselves,
/// and a document given as the wrong type fails.
#[test]
fn each_type_reads_its_own_document() {
    let directory = directory("types");
    let scenario: Scenario = from_yaml(
        &std::fs::read_to_string(fixtures_root().join("vendor-evaluation/journey.yaml")).unwrap(),
    )
    .unwrap();
    let patch = &scenario.steps.as_slice()[0].patch;
    let patch_path = directory.join("patch.json");
    std::fs::write(&patch_path, to_json(patch).unwrap()).unwrap();
    let printed: Patch = from_yaml(&canonical("patch", &patch_path)).unwrap();
    assert_eq!(&printed, patch);

    let graph_text = "\
nodes:
- key: n_plan
  id: plan
  kind: deliverable
  title: Plan
";
    let graph_path = directory.join("graph.yaml");
    std::fs::write(&graph_path, graph_text).unwrap();
    let expected: Graph = from_yaml(graph_text).unwrap();
    let printed: Graph = from_yaml(&canonical("graph", &graph_path)).unwrap();
    assert_eq!(printed, expected);

    let route = fixtures_root().join("hiring-loop/route.yaml");
    for kind in ["scenario", "patch", "graph"] {
        let output = run(&[kind, route.to_str().unwrap()]);
        assert_eq!(output.status.code(), Some(1), "a route file read as {kind}");
    }
}

/// A document the parser refuses exits 1 and prints the parser's own error, which names the
/// path to the failing value: a kind restriction (A1a), a limit, and a key of the wrong type.
#[test]
fn a_refused_document_prints_the_parsers_error_with_its_path() {
    let directory = directory("refused");
    let choices: Vec<String> = (0..=cairn_schema::Limit::ChoiceCountPerDecision.max())
        .map(|choice| format!("  - c{choice}\n"))
        .collect();
    let choices = choices.concat();
    let route = |nodes: &str| format!("format: 1\nroute: sample\nname: Sample\nnodes:\n{nodes}");
    let cases = [
        (
            "route-file",
            route("- id: kickoff\n  kind: milestone\n  title: Kickoff\n  estimate: 2\n"),
            "nodes[0]",
        ),
        (
            "route-file",
            route(&format!(
                "- id: scope\n  kind: decision\n  title: Scope\n  prompt: How wide?\n  answer_type: single_choice\n  choices:\n{choices}"
            )),
            "nodes[0].choices",
        ),
        (
            "graph",
            "nodes:\n- key: r_plan\n  id: plan\n  kind: deliverable\n  title: Plan\n".to_owned(),
            "nodes[0].key",
        ),
    ];
    for (kind, text, at) in cases {
        let path = directory.join("refused.yaml");
        std::fs::write(&path, &text).unwrap();
        let output = run(&[kind, path.to_str().unwrap()]);
        assert_eq!(output.status.code(), Some(1), "{text}");
        let error: ParseError = match kind {
            "route-file" => from_yaml::<RouteFile>(&text).map(drop),
            _ => from_yaml::<Graph>(&text).map(drop),
        }
        .unwrap_err();
        assert_eq!(error.path, at, "{error}");
        assert_eq!(stdout(&output), format!("{error}\n"));
    }
}

/// A request the command cannot carry out exits 2 and prints nothing on stdout: arguments
/// missing or extra, a type it does not know, a file it cannot read.
#[test]
fn a_request_it_cannot_carry_out_exits_2() {
    let route = fixtures_root().join("hiring-loop/route.yaml");
    let route = route.to_str().unwrap();
    let missing = directory("missing").join("absent.yaml");
    let cases: [&[&str]; 5] = [
        &[],
        &["route-file"],
        &["route-file", route, "extra"],
        &["journey", route],
        &["route-file", missing.to_str().unwrap()],
    ];
    for arguments in cases {
        let output = run(arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        assert!(!output.stderr.is_empty(), "{arguments:?}");
    }
}
