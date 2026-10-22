# Touch targets follow the pointer, and the canvas's cards are held by the zoom

Question (the work making controls tappable on phones): when does a control take the 44px minimum hit area, and which controls does the browser test hold to it?

Call:

- The minimum applies under `@media (pointer: coarse)`, not by window width. A phone and a touch tablet both need it; a narrow desktop window with a mouse does not, and keeps its compact controls. Rules that were phone-width literals of 44px (chip remove buttons, the zoom buttons, the sheet's hide button, folded rows) now read the one `--touch-target` token. Layout choices that belong to a narrow window (stacking, the hidden zoom buttons under a raised sheet) stay width queries.
- The test measures the hit area as a finger meets it (the points around a control where a tap lands on it or its label), so padding, a label around a box and a stretched link count and a neighbour that covers a control does not.
- Not held to the minimum: a link inside a run of text, and everything drawn inside the canvas's viewport (cards, edges, their small controls). A card's size is the camera's: at the fitted zoom of a phone a whole journey is on screen and a card is a thumbnail that a tap picks, and zooming (the 44px zoom buttons, a pinch) is the way to make it larger. The controls around the canvas are held to it.

Alternatives: a width query (rejected: it enlarges narrow desktop windows and misses tablets); a minimum zoom on phones so cards are always touch-sized (rejected: a journey would no longer fit the screen); exempting nothing (rejected: no fixed size exists for a scaled drawing).

What would change it: a canvas that lays cards out at a fixed size per screen, or a measured need to pick tiny cards at the fitted zoom without zooming.
