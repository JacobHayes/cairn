# Rung 1's TypeScript checks join with the first TypeScript package

- Question: PRACTICES (The validation ladder) lists a TypeScript typecheck and `eslint` in rung 1, and brief 1.1 pins Node and creates the npm workspace root, but no TypeScript exists yet. Growing the ladder says `check` never depends on a tool nothing uses, and rung 1 fails when a tool reports nothing checked.
- Call: rung 1 runs `rustfmt` and `clippy` (plus a check that every crate inherits the workspace lints). The first brief with a TypeScript package (4.2, `web/client`) adds `typescript` and `eslint` as root devDependencies, the strict `tsconfig` and the `eslint` config (type-aware rules, `max-lines-per-function` at 70, no `any` outside generated code), and a suite for each to `mise-tasks/check/1`.
- Alternatives: install `typescript` and `eslint` now with configs that check nothing, which either fails the nothing-checked rule or needs an exemption from it.
- What would change it: TypeScript landing before 4.2.
