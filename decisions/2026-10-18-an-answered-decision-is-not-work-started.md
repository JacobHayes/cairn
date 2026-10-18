# An answered decision or a reached milestone is not work started beneath a container

Question (from the report that a group stayed "Active" after the node started inside it was stopped): PRD D8 shows a container as `active` when its only unsatisfied waits are its own children "and beneath which work has started", without saying what counts as started. The engine counted any descendant stored `active`, `done`, `decided` or `reached`. Stopping the only started node therefore left the group active whenever an answered decision or a reached milestone sat beneath it, which is most groups once the up-front decisions are answered. Which descendants count?

Call: work has started beneath a container when a deliverable or action under it, at any depth, is stored `active` or `done`. One function, `work_has_started` in `crates/engine/src/derive/display.rs`, cited as D8, is the single reading; the display state and the deprecated group state both use it. An answered decision and a reached milestone open or close gates, and a skipped node did no work, so none of them make a container active. Done work does count: a group with a finished action and nothing running stays active until all of it is finished (then it is ready to finish). PRD D8's row and the `DisplayState` and `GroupState` doc comments say so.

Finding on the stored side: a container never stores a state of its own (no transition applies to a group, and starting a node writes only that node), so nothing goes stale in storage when a child is started, stopped, reopened or skipped. The reported staleness was the derived reading only. The other roll-ups (progress, stage current, badges) are derived from the children on every read and were checked by hand; none reads a recorded start.

Alternatives:
- Keep counting decided and reached nodes: simplest, and a group with an answered decision reads as underway, but every group shows active before anyone has started work, and stopping work never returns it to ready.
- Count only `active` children (not done ones): a group with finished and unstarted actions would read ready between two steps, hiding that it is mid-way.

What would change it: owner feedback that a group with only answered decisions should read as underway.
