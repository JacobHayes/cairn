# Decisions awaiting review

Judgment calls made while implementing the briefs, for the user to review (`AGENTS.md`, Decide, record, keep going). Newest first. Each entry: date, brief, the question, the call, the alternatives, and what would change it.

## 2026-10-06, brief 1.1: rung 1's TypeScript checks join with the first TypeScript package

- Question: PRACTICES (The validation ladder) lists a TypeScript typecheck and `eslint` in rung 1, and brief 1.1 pins Node and creates the npm workspace root, but no TypeScript exists yet. Growing the ladder says `check` never depends on a tool nothing uses, and rung 1 fails when a tool reports nothing checked.
- Call: rung 1 runs `rustfmt` and `clippy` (plus a check that every crate inherits the workspace lints). The first brief with a TypeScript package (4.2, `web/client`) adds `typescript` and `eslint` as root devDependencies, the strict `tsconfig` and the `eslint` config (type-aware rules, `max-lines-per-function` at 70, no `any` outside generated code), and a suite for each to `mise-tasks/check/1`.
- Alternatives: install `typescript` and `eslint` now with configs that check nothing, which either fails the nothing-checked rule or needs an exemption from it.
- What would change it: TypeScript landing before 4.2.

## 2026-10-06, brief 1.1: where the crate-scoped lints apply

- Question: PRACTICES (Code conventions) denies some lints only in some crates (`indexing_slicing` and `HashMap` in the engine, `missing_docs` on public engine items, `panic` in shell handlers), but Cargo's `[workspace.lints]` is one table for every crate, and a crate that inherits it cannot add to it.
- Call: the workspace lints carry the crate-wide set (`unsafe_code` forbidden, `pedantic` warned, `unwrap_used`, `expect_used`, `too_many_lines` at 70 denied). Rung 1 raises the scoped lints on top, by package name, from the moment the crate joins the workspace: `cairn-engine` gets `clippy::indexing_slicing`, `clippy::disallowed_types` (`HashMap` and `HashSet`, listed in `clippy.toml`), and `missing_docs`; `cairn-api` and `cairn-mcp`, the crates whose functions are handlers, get `clippy::panic`. Test code (`#[test]`, `#[cfg(test)]`) may unwrap, expect, index, and panic, through `clippy.toml`'s `allow-*-in-tests`. Rung 1 denies warnings locally as CI does, so `check` means the same everywhere.
- Alternatives: `#![deny(...)]` attributes in each crate's root, which depend on the brief that creates the crate remembering them; a `clippy.toml` per crate, which cannot set lint levels.
- What would change it: handlers living in another crate; wanting `unwrap` denied in tests too; a rust-analyzer user wanting the scoped lints in the editor (then also add the attributes).
