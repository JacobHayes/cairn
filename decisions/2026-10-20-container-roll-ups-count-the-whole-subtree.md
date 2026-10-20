# Container roll-ups count the whole subtree

**Question.** A container shows its progress (done of in scope), an overdue count and a "decision needed" badge. The canvas, the inspector, the Plan list and the Timeline each counted these in the app, over slightly different sets (direct children, or the descendants that the visible level kept). Which set is it, and who counts it? (From moving derived values out of the web app and into the engine.)

**Call.** The engine counts once per read, for every node that has non-group work beneath it (`progress` on the node and on the level's roll-up): every in-scope node beneath it at every depth that is not a group, not stored skipped and not skipped through a container. Done is finished, decided, reached or auto-reached. Decisions needed are decisions that can be acted on now. Empty groups count for nothing; a node that waits on an unanswerable decision to apply at all is out of scope and counts for nothing; a snoozed decision still counts as one to decide. A kept node under a skipped container counts on its own. The "decision needed" badge reads any decision beneath the container, not only a direct child: a hidden node rolls up into its visible ancestor (C2), so a direct-children badge hid a decision under a collapsed or hidden container.

**Alternatives.** Direct children only, C2's literal wording: simpler to say, but wrong whenever depth or a collapse hides the level between, which is when the badge matters most. Counting in the app from the level's visible nodes: rejected, since the counts then change with the view.

**What would change it.** A need for a count of the direct level (a checklist inside a card), which is a separate value with its own name.
