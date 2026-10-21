# Proof: a retried write never undoes a change made meanwhile

A write that went stale is resubmitted on its own only when nothing that changed meanwhile
is something it would write. What it would write now comes from applying it, so a
resubmission can no longer silently undo someone else's change. A stale answer from the API
carries it as `touched`, beside `intervening`.

## A role removal and a fill made meanwhile

One tab removes the role Lead while it is empty; another fills it first. Both start from
revision 1 of the journey. Observed through the service and the client's real retry logic:

| | Before | After |
|---|---|---|
| The removal's stale answer | intervening: Lead's fill | intervening: Lead's fill |
| What the removal was said to write | Lead, its retired key | Lead, its retired key, Lead's fill |
| The client | resubmits it at revision 2 | shows the conflict; nothing resubmitted |
| The journey afterwards | revision 3, Lead gone with its fill | revision 2, Lead filled |

When the fill made meanwhile was another role's, the removal still resubmits on its own
and lands, as before.

## The same, for every write

Any write's stale answer now says what the write would really write: for example,
reopening a node clears its guard bypass, and removing a role or participation kind
rewrites the insertions that map it. A write that would be refused at the new revision,
such as a removal that would now take a step added beneath it, is shown as a conflict
rather than resubmitted. The assistant counts the nodes of a direct write from the same
source, and answers a state write that would be refused with that refusal without running
it.

## Known limits

- Each round compares what intervened since the last resubmission, not since the write was
  first made.
