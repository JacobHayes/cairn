# The sync chip's colour says what it asks of you

Question (from the web redesign, PRD H5, H6, D7): saves raised a toast that had to be closed, and lag, skew, and a dropped stream each had a banner. People asked for one small status: green in sync, yellow stale, red disconnected or conflicted, and whether disconnected and conflicted need different colours.

Call: one chip at the right end of the status strip, with a word always shown. Green means nothing to do (in sync, saved); steel means working on it (saving, updating); amber means safe to wait (behind, reconnecting, offline, new version); red means your change is not landing until you act (not saved, conflict). So disconnected is amber and conflicted is red: they ask different things of you, and the word and dot shape also tell them apart. The view lagging a newer revision is "behind", not "stale", which already names a node flag. Saves show only a brief "saved" on the chip and a receipt at the control with a sentence only for a warning; informational consequences are kept in the chip's popover under Recent. No undo and no offline edit queue for now.

Alternatives: red for offline (it heals itself, so red would cry wolf); repeating informational consequences in the receipt and chip (the verbose toast rebuilt in three pieces).

What would change it: people missing warnings that now appear only at the control, which would bring back a transient sentence on the chip.
