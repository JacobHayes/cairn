//! Writes the rule tables the web app reads to a file: `cairn-rules <path>`. Run by
//! `mise run gen` (mise-tasks/gen/openapi). The engine's tables come first; after them, the
//! patterns of what text a person or an agent reads never cites, which the repository's
//! documents give: the integration tests hold served text to them, and the browser tests
//! hold every screen to them.

use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: cairn-rules <path>");
        return ExitCode::FAILURE;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let citations = match citations(&root) {
        Ok(citations) => citations,
        Err(error) => {
            eprintln!("cairn-rules: reading the documents: {error}");
            return ExitCode::FAILURE;
        }
    };
    let text = cairn_engine::rules::typescript() + &stream_limits() + &citations;
    if let Err(error) = std::fs::write(&path, text) {
        eprintln!("cairn-rules: writing {}: {error}", path.display());
        return ExitCode::FAILURE;
    }
    println!("cairn-rules: wrote {}", path.display());
    ExitCode::SUCCESS
}

/// The limits of the server's change stream the page backs off by, in milliseconds: how
/// often a subscriber is handed ticks, and how long a write may go untaken. The engine crate
/// cannot see them.
fn stream_limits() -> String {
    format!(
        "\n/** The shortest wait before a closed change stream is reopened: the coalescing interval. */\n\
         export const SSE_COALESCING_INTERVAL_MS = {};\n\
         /** The longest wait: the time a stalled subscriber is cut after. */\n\
         export const SSE_WRITE_STALL_MS = {};\n",
        cairn_api::limits::SSE_COALESCING_INTERVAL.as_millis(),
        cairn_api::limits::SSE_WRITE_STALL.as_millis()
    )
}

/// The patterns of what a reader cannot follow, as regular expressions both test suites
/// read (ECMAScript, as JSON Schema's `pattern` is): a requirement id (a letter of one of the
/// PRD's requirement sections, each a `### X.` heading, one or two digits, and an optional
/// lowercase letter, standing alone; a backtick beside it marks code, not a citation) and the
/// name of one of the repository's documents (the Markdown files at its root named in
/// capitals), as a word.
fn citations(root: &Path) -> std::io::Result<String> {
    let prd = std::fs::read_to_string(root.join("PRD.md"))?;
    let sections: String = prd
        .lines()
        .filter_map(|line| line.strip_prefix("### "))
        .filter_map(|heading| match heading.as_bytes() {
            [letter, b'.', ..] if letter.is_ascii_uppercase() => Some(char::from(*letter)),
            _ => None,
        })
        .collect::<BTreeSet<char>>()
        .into_iter()
        .collect();
    let mut documents: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let name = entry?.file_name();
        let stem = name.to_str().and_then(|name| name.strip_suffix(".md"));
        if let Some(stem) = stem
            && !stem.is_empty()
            && stem.bytes().all(|byte| byte.is_ascii_uppercase())
        {
            documents.push(stem.to_owned());
        }
    }
    assert!(!sections.is_empty(), "the PRD has requirement sections");
    assert!(!documents.is_empty(), "the root has documents");
    documents.sort_unstable();
    let word = "A-Za-z0-9_";
    let requirement = format!("(?<![{word}`])[{sections}][0-9]{{1,2}}[a-z]?(?![{word}`])");
    let document = format!("(?<![{word}])(?:{})(?![{word}])", documents.join("|"));
    let mut out = String::from(
        "\n// What text a person or an agent reads never cites, from the repository's documents.\n",
    );
    for (comment, name, pattern) in [
        (
            "A requirement id: a section letter, one or two digits, an optional lowercase letter.",
            "REQUIREMENT_ID_PATTERN",
            requirement,
        ),
        (
            "The name of one of the repository's documents, as a word.",
            "DOCUMENT_NAME_PATTERN",
            document,
        ),
    ] {
        let literal = serde_json::to_string(&pattern).map_err(std::io::Error::other)?;
        let _ = writeln!(out, "/** {comment} */\nexport const {name} = {literal};");
    }
    Ok(out)
}
