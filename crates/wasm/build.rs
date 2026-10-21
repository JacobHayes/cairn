//! Writes `fixtures.rs` into the build directory: one `Fixture` for each directory under
//! `fixtures/` that holds a `journey.yaml`, in name order, with its `route.yaml` when it has
//! one. The directory is the list's one owner: a fixture added there is seeded with no edit
//! here, and a directory with no scenario (a lone segment) is not a fixture.

use std::fmt::Write as _;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&root)? {
        let path = entry?.path();
        if path.join("journey.yaml").exists() {
            names.push(
                path.file_name()
                    .ok_or("a directory has a name")?
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    names.sort();
    let mut out = format!(
        "/// Every fixture, in name order (the order they are seeded).\npub const FIXTURES: [Fixture; {}] = [\n",
        names.len()
    );
    for name in &names {
        let directory = root.join(name);
        let file = |file: &str| format!("{}/{file}", directory.display());
        let route = if directory.join("route.yaml").exists() {
            format!("Some(include_str!({:?}))", file("route.yaml"))
        } else {
            "None".to_owned()
        };
        writeln!(
            out,
            "    Fixture {{ name: {name:?}, route: {route}, journey: include_str!({:?}) }},",
            file("journey.yaml")
        )?;
    }
    out.push_str("];\n");
    let target = Path::new(&std::env::var("OUT_DIR")?).join("fixtures.rs");
    std::fs::write(target, out)?;
    Ok(())
}
