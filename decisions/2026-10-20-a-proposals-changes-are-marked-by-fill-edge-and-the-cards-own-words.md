# A proposal's changes are marked by a fill, an edge and the card's own words, never a badge

Question (from the proposal-layer unit): the review marked changes with a tag hanging off each card, which scaled badly when zoomed out, overlapped its neighbours and was worse on a phone. How are changes marked so they read at every zoom?

Call:

- A fill: `--color-add`, `--color-change` (new: warning at 12% in light, 28% in dark, with a contrast floor for text over it) and `--color-del`. A fill is area, so it reads in a fitted graph.
- An edge in the change's ink, 2px on screen at any zoom, its style the non-colour cue: solid for added, dotted for changed, dashed for removed (the title struck and the card ghosted). DESIGN's 2px rule gains this exception. A container carries the fill on its head band and the edge on its outline.
- An item that needs a choice gets an accent outline outside the card, so it never displaces the change edge, and the picked card keeps its selection treatment.
- The change's word ("Added", "Changed", "Removed") leads the card's own kind line, so it scales with the card and the kind takes the ellipsis first. The card's title and accessible name carry it too. Lists and the timeline fill the row and lead with the word; Entities' rows do the same.
- Marks read unresolved items only: a resolved conflict leaves its added, changed or removed mark and no choice mark. Every node a state mutation names (a pin, an answer, a transition) is marked changed, not only definition changes.
- Removed things are put back where they were, ghosted. A removed node the before level rolled into a kept card marks that card changed instead of drawing a card, and a requirement removed between two kept nodes is drawn once between the cards that stand for its ends.

Alternatives: a hanging badge held constant on screen (the old failure: it outgrows the cards); a badge scaled with the card and hidden when zoomed out (a tag on every card still reads as clutter, worst on a phone); colour alone (fails without it).

What would change it: a mark that must show at a zoom where a 2px edge is itself too heavy.

Superseded by decisions/2026-10-21-a-proposals-change-marks-scale-with-the-card.md: the marks stand; the rule is that a mark scales with its card, not that it is never a badge.
