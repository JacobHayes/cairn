# Proof for brief 5.2: Canvas, semantic zoom, trace, layout

A journey is now a pannable, zoomable flowchart. You can hide kinds of node and see them roll
up into their containers, drill into a container, trace a node's upstream and downstream,
see what ranks first, and tell when a journey has nothing left to act on.

## Screens

Every kind shown: the top two frontier items ranked, irrelevant work grayed, gates dotted:

![The whole journey](1-whole-journey.png)

Actions hidden: the test plan's actions become its checklist:

![Actions hidden](2-actions-hidden-as-checklists.png)

Groups only: everything rolls up into its group:

![Groups only](3-groups-only.png)

Groups and milestones hidden: the final report is marked as waiting on something hidden:

![The hidden-prerequisites marker](4-groups-and-milestones-hidden-marker.png)

Drilled into Setup, with the breadcrumb back out:

![Drilled in](5-drilled-into-setup.png)

The test plan traced across levels, upstream and downstream:

![The trace](6-trace-of-the-test-plan.png)

The heat overlay, showing gravity and leverage as numbers:

![Heat](7-heat-overlay.png)

An offer decision reopened: the work it decides is ghosted as undecided:

![Undecided](8a-undecided-ghosted.png)

Decisions hidden: the work they block carries the marker:

![Hidden decisions](8b-decisions-hidden-marker.png)

The marker opens the trace, naming the hidden decision:

![The marker's trace](8c-marker-opened-the-trace.png)

The only acting item snoozed: the journey says what it waits on:

![Stalled](9-stalled-surface.png)

A route's own canvas, with no journey state:

![A route's canvas](10-route-canvas.png)

The main flow: kinds hidden and shown, Setup drilled into and out, the test plan traced:
[main-flow.webm](main-flow.webm).

Layout is stable: the same journey lays out identically twice, and adding one action under Reporting moved 5 of its 27 nodes, all within their containers.
