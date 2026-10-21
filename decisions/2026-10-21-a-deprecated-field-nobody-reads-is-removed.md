# A deprecated field nobody reads is removed: the largest child gravity

Question (from the work making what agents are told agree with itself): a container's largest
in-scope child gravity was kept on a node's derived values and the level's roll-up, marked
deprecated, after the gravity of the container's whole area replaced it. Is it an engine rule
served for a reason, or a copy kept for clients?

Call: removed, from the engine's priority pass, the schema, the API document and the generated
client. No screen, test of a screen, or agent instruction read it; the only readers were
tests of the value itself and a walkthrough example, which now shows the area's gravity. This
follows the call that removed the group state: Cairn keeps no compatibility field nobody
depends on.

Alternatives: keeping it deprecated until a breaking release (a second gravity number agents
and readers of the API document must tell apart, with nothing depending on it).

What would change it: a client outside the repository that reads it, which would need a
release note and a version step instead.
