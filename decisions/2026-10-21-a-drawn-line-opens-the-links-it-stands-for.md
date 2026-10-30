# A drawn line opens the links it stands for

**Question.** A line on the canvas between two cards can stand for many edges whose ends are hidden inside them. What does selecting it address and show?

**Call.**
- A line's address names the two cards it joins at the view's own level (no node selected, nothing unfolded), on the Graph, for journeys and routes alike. Only lines that level draws are selectable; a line drawn only because of a selection keeps its hover and opens no card.
- The card lists the graph's own edges the line stands for, one row per edge, each end a link, the first 50 shown and the rest offered.
- Whether an edge still holds comes from the dependent's unsatisfied gates as the engine derives them, never from the requirement's display state, which disagrees with blocking (a skipped container with kept work pending still holds; a prerequisite ruled out stops holding without finishing). A route's lines carry no state.

**Alternatives.** An address naming the underlying edges (unbounded for a folded container); every edge between the two cards' subtrees (over-counts edges drawn to children of their own); state from display state (wrong in the cases above).

**What would change it.** The level query taking several drawn nodes for another reason, which would make a selection's own lines cheap to select.
