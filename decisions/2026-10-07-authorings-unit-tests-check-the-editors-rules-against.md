# Authoring's unit tests check the editors' rules against the real engine, and rung 6 builds the module first

- Question: the editors encode rules the engine enforces (which fields a kind has, which comparisons an answer type takes, what a removal must name and rewrite, where an edge is refused). Tested against hand-written expectations, the two could drift unnoticed. Rung 6 ran its Vitest suites before building the wasm module.
- Call: authoring's Vitest tests load the browser host's module in Node, seeded with the fixtures as the in-browser host is, and check each rule against what a local apply accepts (A1a per kind and answer type, A5 per answer type and operator, A3, A8, A18 with and without its cascade, B4 reset and restore). Rung 6 now runs `build:wasm` before its web unit tests rather than after them.
- Alternatives: the same checks in Playwright only (slower, and a rule's failure would surface as a UI symptom); mirrored tables in TypeScript tested against themselves.
- What would change it: the module's build becoming slow enough that the inner loop wants Vitest without it, which would split engine-backed tests into their own suite.
