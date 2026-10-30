# The layout is hinted by the view's last positions; a fresh view lays out from its graph alone

Question: the same graph should lay out identically, and a small edit should move few nodes. ELK's interactive strategies are not hierarchy-aware: on the nested canvas interactive crossing minimization is refused, and interactive layering alone moves every node when one is added. A layout keyed to the previous revision also depends on a tab's history.

Call: layered layout with a fixed seed and ELK's semi-interactive crossing minimization, fed the view's latest positions as hints, so each layer keeps its order across an edit. A layout is cached by everything it reads, not by revision, so a revision that changes nothing it reads lays out nothing. The same graph with the same hints lays out identically, and a fresh view is a function of its graph alone. Adding one node or one requirement moves fewer than a quarter of a graph's nodes within their containers.

Alternatives: model order with no hints (the same in every tab, but about twice the movement); interactive layering (moves every node); positions stored with the graph (pinned positions are a later feature).

What would change it: people noticing two tabs drawing one revision differently, or ELK gaining hierarchy-aware interactive strategies.
