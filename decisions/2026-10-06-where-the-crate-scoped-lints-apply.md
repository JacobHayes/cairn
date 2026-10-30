# Crate-scoped lints are raised by package name in the lint step

Question: some lints apply only to some crates (indexing and hash collections in the engine, missing docs on its public items, panics in handler crates), but Cargo's workspace lint table is one table every crate inherits and cannot extend.

Call: the workspace table holds the lints every crate shares. The lint step raises the scoped ones on top, by package name, from the moment a crate joins the workspace. Test code may unwrap, expect, index, and panic through `clippy.toml`'s test allowances. The local lint denies warnings as CI does.

Alternatives: `#![deny]` attributes in each crate root (they depend on whoever creates the crate remembering them); a `clippy.toml` per crate (it cannot set lint levels).

What would change it: wanting the scoped lints in the editor (then add the attributes as well), or handlers moving to another crate.
