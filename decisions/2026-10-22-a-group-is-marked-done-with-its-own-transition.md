# A group is marked done with its own reasoned transition, and what is left inside reads skipped

Question (from the work letting a stage be finished while work inside it is still open): how
does a person or an agent finish a group, or stage, that still holds unfinished work, and what
does that work read as afterwards?

Call: a new transition, `mark_done`, on groups, taking a reason. It is named for what the
screen says ("Mark done...") and not `close`, because "closed" already means something in
Cairn: a closed journey, and the blocking `closed` flag of a terminal node.

- The group is stored `done`, with the one `reason` field a skip uses. The field was
  `skip_reason`; it is renamed once, in the stored state and its column, with a migration that
  rewrites stored JSON and no alias for the old name.
- Everything unfinished and in scope beneath a group marked done reads skipped, exactly as
  under a skip of the group, through the one cascade: a node's stored state skips what is
  beneath it when it is `skipped`, or when it is a group stored `done`. Stored states inside are
  untouched, finished work stays done, and Keep exempts a subtree. Reopen removes this group's
  mark and nothing else: skips, keeps and marks deeper inside stay as they were.
- Marking done is not guarded (no dependency or relevance check, no bypass), like a skip, and
  never stale.
- No finish date is recorded: a group has no performed dates, and a fixed finish would turn
  kept work that is still open into a shortfall on the group. The date network reads a group
  marked done as a skipped one.
- The terminal shortcut in the blocking sweep is fixed so that kept work holds the group's
  dependents: a group marked done satisfies them only once the kept, in-scope work beneath it
  does. This narrows the call that a `done` node satisfies its dependents whatever was reopened
  beneath it (`2026-10-06-what-satisfies-dependencies-and-what-the-frontier-promises.md`): that
  still holds for a done deliverable or action, but not for a group marked done with kept work
  pending.
- Deliverables and actions with children get no such transition. Completing one is a plain
  move that leaves its children open, because those children are work of their own (order is
  never a guard, `2026-10-24-recorded-dates-never-warn.md`).
- The screen offers Mark done in the inspector's overflow menu beside Skip, never as a primary
  action and never in bulk. A group marked done shows "Marked done" with how many items it
  skipped in place of its progress meter (a full meter over only the unskipped work would read
  as all of the work finished), or "Kept work pending" while kept work holds its dependents.
- The derived values replace the `effectively_skipped` flag with `skipped_by` (the ancestor
  whose skip or marking done skips the node), and node detail gains `kept_work_pending`, so the
  inspector can say what still holds the dependents.

Alternatives: a reasoned `complete` (closing differs in kind: it is unguarded, performs no work,
records no finish and starts a cascade, so a reasoned `complete` needs special cases in the
guards and the bypass); a second column for a mark's reason, or a serde alias for the old name
(two sources of truth); a "done anyway" for groups that leaves the children open (it would hide
open work under a done stage); recording a finish date.

What would change it: people wanting a stage's leftover work to stay visible as open, or wanting
a date on the mark.
