# The engine's rule tables are generated for the app

**Question.** The app kept hand copies of the engine's limits, the states each kind's moves leave from, the initial state of each kind and the terminal states, plus a second file of limit constants. Each drifted silently from `crates/schema` and `crates/engine`. How does the app read them? (From moving derived values out of the web app and into the engine.)

**Call.** They are a generated artifact: `web/client/generated/rules.ts`, written by `mise run gen` from the engine's own tables (a small binary in the api crate calls `cairn_engine::rules::typescript`), exported from the client package, checked in and regenerated in the same commit as any change to a table, like the OpenAPI types. The app deletes its copies and its limit constants and imports these. The generated step of the ladder keeps the file honest.

**Alternatives.**
- Keep hand copies with engine-backed parity tests: keeps the duplication and the tests pin the copies, not the rule.
- Ask the engine through a wasm call: asynchronous, and absent from pure unit tests of the app's models.

**What would change it.** A table that depends on per-journey data (it would then be a derived value, not a rule table).
