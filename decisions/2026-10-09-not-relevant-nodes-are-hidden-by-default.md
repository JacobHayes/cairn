# Not-relevant nodes are hidden by default; conditional nodes are shown

Question (from the web redesign, PRD C1): C1 grayed out not-relevant nodes and ghosted undecided ones, neither hidden by default. People found settled-out branches clutter the graph and lists.

Call: settled not-relevant nodes are hidden by default on every surface, with their count always shown ("4 not relevant hidden", with Show) and a filter to reveal them; a node you navigate to (a link, search, or a trace's Reveal) is revealed ghosted. Conditional nodes, pending ones included, are shown ghosted (dashed, steel text, "if X = Y") and rolled up at the Stages step, with a filter to turn them off; they never count as hidden. The display set is part of the engine's level request (`2026-10-09-one-drill-mechanism-and-two-rules-for-what-is-open.md`), so a hidden not-relevant node rolls up and re-targets its edges like any other hidden node. This replaces the tab-side filter in `2026-10-07-hiding-not-relevant-or-undecided-nodes-keeps-c2s-hidden.md`.

Alternatives: hiding conditional nodes too (they are exactly what an open decision brings in or rules out, so seeing them makes the decision meaningful); keeping not-relevant nodes shown and grayed (the clutter people asked to remove).

What would change it: routes where conditional branches dominate early, which would argue for rolling them up further rather than hiding them.
