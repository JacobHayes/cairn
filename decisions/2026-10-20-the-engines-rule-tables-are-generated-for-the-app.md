# The engine's rule tables are generated for the app

**Question.** The app needs the engine's limits, each kind's initial and terminal states, the moves each journey status allows and whether it accepts changes or holds current work, the words each display state is said by, and the patterns served text must never contain. Hand copies drift silently. How does the app read them?

**Call.** The engine and schema own them, and `mise run gen` writes them into `web/client/generated/rules.ts`; the app imports them and keeps only design choices such as badge tones. A change to a table regenerates the file in the same commit, and the generated step catches a stale one. The served agent guide, written by hand, is held to the display-state words by an integration test, and both test suites read the citation patterns from the same generated file.

**Alternatives.** Hand copies with parity tests (the tests pin the copies, not the rule); asking the engine through a wasm call (asynchronous, and absent from pure unit tests); serving the words through the API (no client but the app reads them).

**What would change it.** A table that depends on per-journey data, which would then be a derived value, not a rule table.
