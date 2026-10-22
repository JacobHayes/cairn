# Work waiting for its date reads scheduled

Anything a date ahead holds now reads Scheduled and leaves the next list: a milestone pinned
to a future day or marking itself done on it, and work or a decision whose own not-before
date is still ahead. What waits on it is blocked and says the date. Work can still be started
early; finishing it before its date needs a reason, and the reason records the date.

The launch fixture, read on Oct 6: the beta's start (marks itself done Nov 13), the launch
(pinned Nov 23) and the announcement post (not before the beta ends, Nov 11) are scheduled.

## In the list

A scheduled row says its date under the title; on a phone that date is its one secondary fact.

![The list: Announcement post, Scheduled, Not before Nov 11](1-list.png)

![The same row on a phone, dark](6-list-phone-dark.png)

## In the inspector

The sentence says when, and how to go early: Start is offered, Mark done is held, and Done
anyway (or Answer anyway, for a decision) takes a reason.

![Announcement post: Scheduled, not before Nov 11; it can be started early](2-inspector-scheduled.png)

What waits on a scheduled node names its date, in the sentence and under Connections.

![Beta, blocked: waiting on Beta starts, scheduled for Nov 13](3-inspector-waits.png)

## On the Summary

A folded Scheduled panel lists the work and decisions waiting for their date; milestones stay
in the Milestones panel.

![Summary: Scheduled, folded](4-summary-folded.png)

![Summary: Scheduled opened, Announcement post, Not before Nov 11](5-summary.png)

With everything ready snoozed, the journey is held only by dates and snoozes, and the stall
panel names each: Start for work, Mark done for a milestone.

![Stalled: the announcement not before Nov 11, the beta's start and the launch](7-stalled.png)

The flow, list to inspector to Summary: [scheduled.webm](scheduled.webm)

## Known limits

- A milestone dated only by its deadline (a "due by" date) is not scheduled: it reads ready
  and is marked done when it happens.
- A container's own date never holds its children.
