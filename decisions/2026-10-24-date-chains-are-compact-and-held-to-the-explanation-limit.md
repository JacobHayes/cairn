# Date chains are compact and held to the explanation limit

Question: a date chain's constraints can be written as nested objects or compactly, and every other explanation list is bounded. What shape do chains take, and how are they bounded?

Call: one compact shape everywhere (server, agent tools, rejections, the app's local derive). A constraint is `after`, `before`, `days`, `source`, and `conditional` when it holds only under a condition; an instant and a source are each one string that still names every node involved. Node detail carries at most the explanation entry limit per chain with a `total`, and the explanation reads page the rest from a cursor. A rejection's chains are not cut, since no later read can continue a refused patch. The app's local derive keeps whole chains. No new limit.

Alternatives: compact only in the agent tools (two shapes for one type); dropping node fields a source's ends imply (the ends do not always name the responsible node, and the form could not be read back); cutting rejection chains (unrecoverable).

What would change it: a client that needs the nested form, or a chain whose fixed dates grow past the limit.
