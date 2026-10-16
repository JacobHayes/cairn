//! Rebuilds the binary when the web build changes, so `cargo build` after `mise run
//! build:web` always embeds the build it made (rust-embed tracks only the files it saw).
//! With the `web-dir` feature nothing is embedded, so the web build is no input at all.
//!
//! `rerun-if-changed` reruns this script, but a build cache keyed on the compiler's
//! command and inputs (mbx) does not count it as an input of the crate's compile, and
//! restores the compile made from the old folder. So the script also hands the compile a
//! digest of the folder in `CAIRN_WEB_BUILD_DIGEST`, which `src/assets.rs` reads with
//! `env!`: a different web build is then a different compile for Cargo and the cache alike.
//!
//! The script also hands the compile the version the binary reports, in
//! `CAIRN_BUILD_VERSION`: the release tag in `CAIRN_VERSION` (`v1.2.3`), or `dev` when it is
//! unset, and a build error when it is not a semantic version. The crate version stays
//! `0.0.0`; a release takes its version from its tag (ARCHITECTURE, Build, run, deploy).

use std::env::VarError;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::path::Path;

#[path = "src/version.rs"]
mod version;

/// The web build the binary embeds, relative to this crate.
const WEB_BUILD: &str = "../../web/app/dist/build";

fn main() -> io::Result<()> {
    println!("cargo::rerun-if-env-changed=CAIRN_VERSION");
    let tag = match std::env::var("CAIRN_VERSION") {
        Ok(tag) => Some(tag),
        Err(VarError::NotPresent) => None,
        Err(VarError::NotUnicode(_)) => {
            println!("cargo::error=CAIRN_VERSION is not UTF-8");
            return Ok(());
        }
    };
    match version::from_tag(tag.as_deref()) {
        Ok(release) => println!("cargo::rustc-env=CAIRN_BUILD_VERSION={release}"),
        Err(problem) => {
            println!("cargo::error={problem}");
            return Ok(());
        }
    }
    if std::env::var_os("CARGO_FEATURE_WEB_DIR").is_some() {
        println!("cargo::rerun-if-changed=build.rs");
        return Ok(());
    }
    println!("cargo::rerun-if-changed={WEB_BUILD}");
    let mut hasher = DefaultHasher::new();
    let folder = Path::new(WEB_BUILD);
    let present = folder.is_dir();
    present.hash(&mut hasher);
    if present {
        hash_files(folder, "", &mut hasher)?;
    }
    let digest = hasher.finish();
    println!("cargo::rustc-env=CAIRN_WEB_BUILD_DIGEST={digest:016x}");
    Ok(())
}

/// Hashes every file under `directory` in name order: its `/`-separated path under the
/// build (`prefix` is the path of `directory`), its size, and its bytes.
fn hash_files(directory: &Path, prefix: &str, hasher: &mut DefaultHasher) -> io::Result<()> {
    let mut entries = fs::read_dir(directory)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = format!("{prefix}{}", entry.file_name().to_string_lossy());
        let entry_path = entry.path();
        if fs::metadata(&entry_path)?.is_dir() {
            hash_files(&entry_path, &format!("{path}/"), hasher)?;
        } else {
            let bytes = fs::read(&entry_path)?;
            path.hash(hasher);
            bytes.len().hash(hasher);
            bytes.hash(hasher);
        }
    }
    Ok(())
}
