//! Writes `conformance_suite.rs` into the build directory: the `conformance_suite!` macro,
//! with one case for each `pub async fn NAME<B: Backend>` in a module under
//! `src/conformance/` (the case modules, in file-name order, each in source order). The
//! functions are the list's one owner: a case written there runs on every backend with no
//! edit elsewhere, and a case cannot be left out.

use std::fmt::Write as _;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/conformance");
    println!("cargo:rerun-if-changed={}", directory.display());
    let mut files: Vec<_> = std::fs::read_dir(&directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .filter(|path| path.file_stem().is_some_and(|stem| stem != "mod"))
        .collect();
    files.sort();
    let mut cases = String::new();
    for file in &files {
        let module = file
            .file_stem()
            .ok_or("a file has a name")?
            .to_string_lossy();
        let source = std::fs::read_to_string(file)?;
        for line in source.lines() {
            let Some(rest) = line.strip_prefix("pub async fn ") else {
                continue;
            };
            let Some((name, generics)) = rest.split_once('<') else {
                continue;
            };
            if generics.starts_with("B: Backend>") {
                writeln!(cases, "            {module}::{name},")?;
            }
        }
    }
    if cases.is_empty() {
        return Err(format!("no conformance case found under {}", directory.display()).into());
    }
    let macro_source = format!(
        r"/// Expands to one `#[test]` per conformance case, each running the case against
/// `$backend` on the executor `$run` (a function from a future to its output). The cases are
/// the `pub async fn`s over a `Backend` in the case modules (build.rs).
#[macro_export]
macro_rules! conformance_suite {{
    ($backend:expr, $run:path) => {{
        $crate::conformance_suite!(@cases $backend, $run;
{cases}        );
    }};
    (@cases $backend:expr, $run:path; $($module:ident :: $case:ident),* $(,)?) => {{
        $(
            #[test]
            fn $case() {{
                $run($crate::conformance::$module::$case(&$backend));
            }}
        )*
    }};
}}
"
    );
    let target = Path::new(&std::env::var("OUT_DIR")?).join("conformance_suite.rs");
    std::fs::write(target, macro_source)?;
    Ok(())
}
