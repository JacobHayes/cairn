# The Filter wears a badge of what it hides, without the not relevant

Question (from the filter-badge unit): the toolbar's Filter badge had been removed. The owner wanted the badge kept, minus the not-relevant nodes, which are hidden by default and so noise; and a breakdown per option inside the open Filter.

Call: the Filter badge counts the nodes the viewer's Filter settings hide, never a node settled as not relevant (the screen specs say where it shows and what it counts). Search is not a Filter setting and is not counted. A flag's or the owner's count is what that setting alone hides, among the nodes the search keeps.

The engine's level owns every number. It already counted per class; it now also counts, when the query carries a flag or an owner, the nodes of a kind shown, under no collapse and not selected, that the search keeps and a setting leaves out: in all (each node once) and per setting alone. A settled not-relevant node is left out of these counts whatever the viewer shows. The screen only adds the conditional class's count to the engine's total where the one per-screen table (`SCREEN_VIEWS`) says the view draws that class, and passes each count to its setting. Changing the Filter's counts is therefore one change in the engine.

This supersedes the earlier decision's call that the toolbar wears no badge; its per-class counts beside the boxes stand.

Alternatives: counting in the client from the drawn nodes and the full list (a second computation of what the filter hides, which would drift from the level); counting not-relevant nodes in the badge (noise: it would read as a problem on every journey); counting search in the badge (search has its own box and chip).

What would change it: people reading the badge as hidden problems when the count is only filtered-out work they chose to exclude, which would argue for badging only the settings that hide kinds of work no one chose.
