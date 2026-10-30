# The Filter's badge counts what it hides, without the not relevant

Question: what does the toolbar's Filter badge count, and who computes it?

Call: the badge counts the nodes the viewer's Filter settings hide, never a node settled as not relevant (hidden by default, so noise), and not search, which has its own box and chip. Each setting's own count is what that setting alone hides among the nodes the search keeps. The engine's level computes every number; the screen only adds a class's count where the one per-screen table (`SCREEN_VIEWS`) says the view draws that class. Changing what the Filter counts is one change in the engine.

Alternatives: counting in the client from the drawn nodes (a second computation of what the filter hides, which drifts from the level); counting not-relevant nodes (it would read as a problem on every journey); counting search.

What would change it: people reading the badge as hidden problems when it counts only work they chose to exclude.
