# A container shows the gravity of its whole area

Question (from the web redesign, PRD Priority, C2): a stage's card showed its own gravity (15.5) beside "gravity up to 21.5" for its heaviest child, because containment makes every child's gravity at least its parent's. Two numbers on one card, one apparently larger than the whole, read as a bug. Show the peak, the own gravity, or something else?

Call: one number, `subtree_gravity`: the counted weight of every node in the container's area, the container and its open, in-scope descendants, plus everything downstream of any of them, each node once, under the same pruning and terminal rules as gravity. It is at least every member's gravity, a dependent shared by two children counts once, and a container whose work is all done shows 0. Node `gravity`, leverage, and rank are unchanged, so a deliverable with children is still ranked on its own gravity. The field is added beside `gravity` on a node's derived values and the level's roll-up. `peak_gravity` is removed (added earlier in the same unreleased stack): no screen needs a "heaviest item inside" pointer, and the trace and the list already find heavy nodes. `max_child_gravity` stays, deprecated.

Alternatives:

- Keep `peak_gravity` as well: a second container number nobody shows, and the one that caused the confusion.
- Sum the children's gravities: double counts any dependent two children share.

What would change it: a screen that wants to point at the heaviest node inside a container, which would bring back a named peak beside this number.
