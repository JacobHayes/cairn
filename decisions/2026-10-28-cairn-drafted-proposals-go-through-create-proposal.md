# Cairn-drafted proposals go through create_proposal

Question (from simplifying the MCP tool surface): an upgrade, saving a journey as a route, and a re-link are always proposals Cairn drafts, and each had its own tool (`upgrade`, `save_as_route`, `relink`) beside `create_proposal`, which took the caller's own draft. Four tools made proposals, and three of them were used rarely. Should they stay separate?

Call: `create_proposal` makes every proposal. Its arguments name exactly one content: the caller's own draft with its destination, or one of `upgrade` (the journey and the route version to move it to), `save_as_route` (the journey, the new route's id and name) or `relink` (the journey and the route to link it to). The listed schema is a plain object with each content an optional property, since Anthropic and OpenAI refuse a union at the root of a tool's arguments; the server refuses a call that names none or more than one, at the path of the second content named, and a content named as null is refused at its path rather than read as absent. The three tools are gone; what each drafted, and how it is reviewed and applied, is unchanged.

Alternatives: keep the three tools (three more schemas listed, one more way to start a proposal for an agent to choose between); fold them into `apply_patch` as ops (they are drafts Cairn computes, not mutations a caller writes, and the result is a proposal to review, not an applied patch).

What would change it: agents sending mixed arguments often, which a root union would have shown them; a Cairn-drafted proposal whose inputs no longer fit beside the others.
