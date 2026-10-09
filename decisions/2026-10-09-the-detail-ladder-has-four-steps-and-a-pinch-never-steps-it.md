# The graph's detail ladder has four steps, and a pinch never steps it

Question (from the web redesign, PRD C2): semantic zoom was per-kind checkboxes, which people found confusing and which said nothing about depth. What sets how much of the graph is drawn, and does zooming change it?

Call: a ladder of four steps, Stages (top-level groups and milestones), Decisions (adds decisions, nested groups, and nested milestones), Work (adds deliverables, actions as a progress roll-up), and All, on keys 1 to 4, with Stages left out when there are no top-level groups. Picking out one kind is a filter that fades the others. Zoom changes only how much each card draws, through three bands set on the canvas root in CSS (full card, head and title, title with a shape glyph), so a pinch causes at most three style changes and never re-lays out.

Alternatives:

- Zoom-driven steps (pinch past a threshold to expand): re-laying out mid-gesture breaks spatial memory.
- Keeping kind checkboxes behind a menu: the cause of the confusion, kept alive.

What would change it: very large journeys where Stages still shows too many cards, which would call for a further roll-up step above it.
