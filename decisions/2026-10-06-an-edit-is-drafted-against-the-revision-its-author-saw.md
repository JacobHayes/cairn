# An edit is drafted against the revision its author saw

Question: a stale write is refused and shown to its author. But a UI that names, at send time, the revision it holds then would never be stale on a field someone else changed while the author typed, since the live view has already moved on.

Call: an edit records the revision its author saw when they started editing, and the write names that revision. A change since makes it stale, and the author reviews it and decides again.

Alternatives: the revision held at send time (two people editing one field silently overwrite each other).

What would change it: a form editing several fields at once, which keeps one base revision for the form.
