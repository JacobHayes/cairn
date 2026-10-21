# Tests take engine answers, and run-every-case lists derive from their sources

Question (from the rule-tooling unit): web test supports held hand-written engine answers that include states the engine cannot produce (for example a ready action that its container blocks, or an undecided node with no condition), and several lists that drive "run every case" or seed the fixtures were kept by hand beside the thing they list. Which is the owner of each?

The call:

- The acting screens' shared journey is built by patching a new journey in the real engine and deriving it there; its derived values are the engine's.
- The canvas and node-detail journeys stay as hand-written documents, because their tests assert exact dates, chains and ranks, but a test now derives each one's graph and state again in the engine and requires every node's display state, relevance, actionability, blockers and failing guards to match. Four hand-written values the engine contradicts were corrected, and the tests that leaned on them follow the engine.
- The fixtures the in-browser host seeds are the directories under `fixtures/` with a scenario (build script of the wasm crate); the web test that walks every fixture journey reads them from the seeded host.
- The store conformance cases are the case functions in the conformance modules (build script of the store crate). The API document test builds its request paths from the endpoint table. The generated test documents draw each kind's states from the schema's legality table. The ladder's per-step test modules are one list the unit step skips from, and a step file that is in no part fails the ladder. The agreement-cases inputs come from the manifests.
- Pins that must be written in more than one manifest (the patina revision, the toolchain, the testbeds' lints and rust-version, the function-length limit) cannot be read from one owner by the tools involved, so the lint step checks every copy against its owner (scripts/pins).

Alternatives: derive every test support view from the engine (the exact-value tests would be rewritten around engine output, a large change for little added safety once the agreement test holds); keep the lists and add reminders (they drifted).

What would change it: a screen test that needs a state the engine cannot reach, which would be recorded here with the reason.
