# The editors read the engine's rule tables, and the preview refuses what they used to

Question: the authoring editors re-implemented engine rules (which fields a kind has, which operators an answer type takes, which nodes a reference may name, which roles fit a slot, the id and key formats, the condition size limit). Where do those rules live, and how does an editor ask?

Call: each rule has one owner in the engine or schema, and an editor asks in one of three ways. A fact a picker needs is a table `mise run gen` writes to `web/client/generated/rules.ts` (fields per kind and answer type, flag defaults, leaf kinds, condition operators per answer type, reference targets per field, which roles a slot or a decision takes, the id slug pattern, key prefixes, journey fields, the SSE timings). A question about a value the user is building (is this condition too large) is a derive-worker call (`condition_problem`). A structural refusal (an edge to an ancestor, a condition on the node's own subtree) is not checked by the editor at all: the preview already runs the engine and shows its violation at the field. The pickers offer what the table allows and no more.

Two things stay in the app on purpose, as the shape of a picker rather than a rule: a picker leaves out the node being edited, and `moveTargets` leaves out the node and its descendants (a tree has no such position to choose). A decision inside the node's own subtree is still listed, last and marked, and the engine refuses it.

Behaviour that changed where an editor had disagreed with the engine: a condition of 16 comparisons is accepted (the old count included combinators); an empty "is one of" is no longer refused by the editor; the proposal editor offers only roles that fit a decision's answer type for the role it fills.

Alternatives: keep the copies and test them against the engine (a parity test per copy, which is what existed and kept drifting); serve every table through the OpenAPI document (it is not a type, so the generated table file is the better home).

What would change it: a rule an editor needs that depends on the whole graph and so cannot be a table or a small derive call would make a preview-only answer the pattern for it.
