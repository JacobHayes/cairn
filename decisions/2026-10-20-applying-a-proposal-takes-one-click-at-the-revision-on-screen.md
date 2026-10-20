# Applying a proposal takes one click, at the revision on screen

Supersedes `2026-10-07-applying-a-proposal-needs-the-reviewers-confirmation.md`.

Question (from the proposal-layer unit): the review screen asked for an "I have reviewed this proposal" confirmation before Apply, so a refresh would ask for it again (I6, renewed review). It reads as a legal formality. What stands in for it?

Call: no checkbox. Apply is one click whenever nothing holds it (an unresolved choice, a stale destination, unsaved edits, a candidate that breaks a rule, no preview yet), and it sends as `reviewed_revision` the proposal revision whose preview the layer has finished deriving (the engine's answer for the edits on screen and, for a journey, the journey it leaves, derived in the worker), never the newest one read. The views then draw that derivation a moment later; the server's revision check is the backstop for the gap. One pure function, `applyRevision(latest, drawn)`, gives the revision to send, or none while the drawn snapshot is not the latest, in which case Apply is held as "Previewing...". A proposal edited or refreshed by anyone after the page drew it is held until redrawn or, if the edit lands between press and commit, refused by the server's revision check (H5, I6), and the page redraws. Renewed review is the reviewer seeing the refreshed proposal drawn. A held Apply is `aria-disabled`, lists its holds in its `title`, and pressing it shows the rail and focuses its "Apply waits for" line.

Alternatives: keep the checkbox (the owner's rejection); a stored server-side review record of who reviewed which revision.

What would change it: a requirement that review be recorded per person, which would move it to the host and let another person's review count.
