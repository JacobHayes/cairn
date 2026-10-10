# A browser test is for what only a browser catches

The question: the Playwright suites had grown to 121 tests (about 40 s wall, 160 s of CPU) and most repeated a rule an engine scenario, an API test, or a Vitest already holds, through the same hook. Which belong in a browser (the e2e audit, applied with the change that did).

The call: a browser test stays only for real layout, scroll, focus and gestures; wiring across modules the unit tests mock; the server host's live updates, conflicts and sign-in; and the shipped binary. One test per distinct write path (inspector, bulk bar, triage pass, route authoring, journey edit mode, proposal review, segment stepper, assistant), and no matrices. That left 21 tests (16 in-browser host, 5 on `cairn demo`). What was cut is held where a break is cheapest to catch: a rule that lived in React state (a draft keyed by node, Save's enabling) is pulled out as a pure function and tested with plain Vitest, with no DOM environment added. The browser host's agreement with the server (web/wasm) moved from Chromium to Vitest in Node over the same wasm32 binary: Node's V8 catches the drift only that target has (usize, floats, the stack), and the worker in a real browser stays covered by the in-browser smoke test (the user approved this; brief 4.5 says so).

Alternatives: keep one big Playwright test per suite (still starts a browser and a Vite server for what Node does), or add happy-dom and Testing Library for the React-state cases (a second test environment for about eight rules).

What would change it: a bug in a rule that now lives only in a component's wiring that no kept flow reaches, or a wasm32 difference between Node's V8 and a browser's engine.
