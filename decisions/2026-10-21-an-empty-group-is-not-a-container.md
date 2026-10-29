# An empty group is not a container

Question (from the rule-journey-screens unit): the glossary and the engine call a node a container when it has children, and the list draws an empty group as a leaf. The node menu and the gravity explanation also counted every group, so an empty group offered "Snooze branch" and "Skip branch" and explained its gravity as "this whole area". Which is it?

Call: a container is a node with children, everywhere. The node menu's branch wording and the gravity explanation read the node's children. An empty group still offers Insert route here, since a group exists to hold work and a route is a way to fill it; that is where a route may go, not whether the node is a container.

Alternatives: counting every group as a container (the engine derives nothing for an empty one as a container, so the words would promise a subtree that is not there).

What would change it: groups gaining container behaviour of their own in the engine (a group's own rollup with no children), which would make kind part of the rule.
