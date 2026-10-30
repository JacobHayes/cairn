# Touch targets follow the pointer, and the canvas's cards are held by the zoom

Question: when does a control take the minimum touch hit area, and which controls are held to it?

Call: the minimum applies under `@media (pointer: coarse)`, read from the one `--touch-target` token, not by window width: a phone and a touch tablet need it, and a narrow desktop window with a mouse keeps its compact controls. Layout choices that belong to a narrow window stay width queries. The hit area is measured as a finger meets it, so padding and a surrounding label count and a covering neighbour does not. A link inside running text and everything drawn inside the canvas's viewport are not held to it: a card's size is the camera's, and zooming is how a card gets larger. The controls around the canvas are held to it.

Alternatives: a width query (enlarges narrow desktop windows and misses tablets); a minimum zoom on phones (a journey would no longer fit the screen); exempting nothing (a scaled drawing has no fixed size).

What would change it: a canvas that lays cards out at a fixed size per screen.
