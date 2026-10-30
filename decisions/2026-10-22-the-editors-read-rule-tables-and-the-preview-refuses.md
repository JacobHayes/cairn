# The web app asks the engine for every rule, and the preview refuses

Question: the web app's pickers and forms depend on engine rules (which fields a kind has, which operators an answer type takes, which moves a node accepts, what a removal leaves dangling, which stages are current, what a form value may be). Where do those rules live, and how does a screen ask?

Call: each rule has one owner in the engine or schema, and a screen asks in one of three ways.

- A fact a picker needs is a table `mise run gen` writes to `web/client/generated/rules.ts`. Pickers offer what the table allows and no more.
- A question about the graph or a value being built is a derived value or an engine call through the derive worker: a node's `moves` (what a patch of just that move would be accepted for), current stages, the final milestone, when a snooze lifts, a removal's plan and its rewrites, the placeholders in a message draft, and every form's pre-send checks. These calls are asynchronous, so nothing is asked per keystroke on the main thread.
- A structural refusal is not checked by the editor at all: the preview runs the engine and shows its violation at the field.

The web keeps only facts about its own screen: what to leave out of a picker (the node being edited, its descendants as move targets), which of the accepted moves are worth offering, and the address-to-page mapping.

Alternatives: keeping the copies with a parity test each (copies drift from the engine); serving the tables through the OpenAPI document (a table is not a type).

What would change it: a rule that needs the whole graph and so cannot be a table or a small engine call, which would make a preview-only answer the pattern for it.
