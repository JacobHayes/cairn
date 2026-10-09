# Below 720px a gesture surface is an inert preview that opens full screen

Question (from the web redesign, DESIGN Responsive): below 720px the document scrolls, and an interactive graph or timeline inside it captures two-finger scroll and touch drags, so the page stops scrolling whenever a finger lands on the map. From 720 to 1099px the inspector dropped below the workspace, which made the document scroll with the canvas inside it, the same trap.

Call: from 720 to 1099px the inspector is a bottom sheet, so the document never scrolls. Below 720px a graph or timeline renders as a fitted, static preview with no gesture handlers, which a swipe scrolls past like an image, and one Open map button opens a full-screen layer where gestures belong to the canvas; Esc or its close button returns to the page at the same scroll position, and the address gains `map=1` so Back closes it. A 390px touch check guards it.

Alternatives: per-gesture heuristics on a nested canvas (fragile, and a scroll that sometimes pans is worse than one that never does); a shorter interactive canvas (still a trap).

What would change it: a browser gesture model that lets a nested surface yield vertical scroll reliably.
