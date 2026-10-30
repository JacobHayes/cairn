# The structural checks read Chrome's layout snapshot, probing points only where boxes cannot answer

Question: the screen-structure checks (containment, cut text, touch size, native tooltips, type floor) all need the page's layout. Where do the facts come from?

Call: one reader, `web/app/e2e/snapshot.ts`, takes Chrome's layout snapshot once per state, and every rule is a plain, deterministic function over it, with no model in the loop. Only what needs live state stays in the page: popover state, focus walks, and point probing for a control the boxes cannot settle. A control passes its touch size from its box only when it is at least the minimum each way, drawn whole in the window and not painted over; anything small, covered, partly off screen, clipped, rotated or skewed is probed by points.

Alternatives: keeping in-page geometry beside the snapshot (the same facts twice, drifting); probing every control by points (exact, but it would dominate the phone screenshot walk).

What would change it: a Chrome release that drops a snapshot field a rule reads, or a rule that needs hover or focus state.
