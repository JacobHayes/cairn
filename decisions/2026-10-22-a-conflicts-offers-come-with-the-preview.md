# A conflict's offers come with the preview

Question: the proposal review kept a hand copy of `Conflict::offers` (what resolutions a conflict offers and which choices an answer lost), held to the engine by a parity test (audit of the authoring editors).

Call: `Conflict::offered()` lists the kinds of resolution in the order a reviewer reads them and `offers(resolution)` is that list plus the existing choice-map check. The preview carries `conflicts`, one entry per conflict item with its offers and, for an answer conflict, the choices the route no longer has. The review screen draws those; the copy, the parity test and the stand-in list of unresolved items are deleted. While a reviewer's edits are newer than the preview, the unresolved marks wait for the preview rather than being guessed. What a conflict offers does not depend on the edits, so it is always available.

Alternatives: generate a table from the conflict kinds (offers depend on the conflict's content, such as an insertion or a role's width, so a table cannot hold them).

What would change it: a conflict whose offers depend on the reviewer's other choices would need the offers per edit, not per item.
