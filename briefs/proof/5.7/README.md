# Proof for brief 5.7: Proposal review and its flows

A proposal can now be reviewed in the browser as a diff over the canvas and as editable
lists, then applied or discarded; upgrades, saves as route, re-links, and breakdowns open one.

![The overview's proposals](1-overview-proposes.png)
The overview offers the upgrade to version 2, a save as route, and a re-link.

![An upgrade as a diff](2-upgrade-as-a-diff.png)
The upgrade on the canvas: the sign-off added, two conflicts, an orphan, and a kept edit.

![Conflicts resolved](3-conflicts-resolved.png)
One conflict keeps the journey's title, one takes the route's condition; the orphan goes.

![Applied](4-applied.png)
Saved, confirmed as reviewed, and applied.

![Stale](5-stale.png)
The journey moved after drafting. The proposal lists the change and holds the apply.

![Refreshed](6-refreshed-review-again.png)
Refreshed against the journey; apply waits for a new review.

![Save as route](7-save-as-route-mapped.png)
The routeless journey saved as a route, its owner mapped to a new role.

![Re-linked](8-relinked.png)
After publishing, the journey is re-linked and follows the new route.

![Break down from triage](9-break-down-from-triage.png)
A placeholder's triage card takes two pieces and opens them as a proposal.

![The breakdown and its frontier](10-breakdown-with-its-frontier.png)
The breakdown's review: two nodes added, both newly on the frontier.

![Dark theme](11-dark-theme.png) ![Narrow screen](12-narrow-screen.png)
Review in the dark theme and on a narrow screen.

[main-flow.webm](main-flow.webm): the upgrade proposed, its conflicts and orphan resolved,
applied, and the journey opened.

## Known limits

- Proposals have no index: one is reached by its link, which every flow opens.
- Edits to a route or deployment proposal are previewed once saved; a journey's as typed.
- The review confirmation is kept in the tab; version 2 raises only field conflicts and
  orphans, so other conflict kinds are checked in unit tests.
