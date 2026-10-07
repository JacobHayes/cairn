# Proof for brief 5.6: Authoring

A route's structure can now be built by hand on its canvas, from an empty draft, and a
journey's structure edited in an edit mode on its canvas: nodes, edges, conditions, date
rules, stages, roles, resources, breakdowns, and removals with their cascade, each previewed
by the engine so a problem shows at its field before anything is sent. The pictures and video
are made by `prove.sh` on the in-browser host.

![An empty draft](2-empty-draft.png)
A new route starts with an empty draft; the palette adds nodes to it.

![A decision fills a role](3-a-decision-fills-a-role.png)
A decision's form offers only a decision's fields, and fills a role.

![A condition](4-a-condition.png)
A condition picks a decision and a comparison its answer type takes.

![A date rule](5-a-date-rule.png)
A due-by rule measured 10 days after the kickoff milestone.

![An edge refused](6-an-edge-to-its-own-stage-refused.png)
A requirement drawn to the node's own stage is refused with the reason.

![The route](7-the-route-on-its-canvas.png)
The authored route: a stage opened by a milestone, a condition, a drawn requirement.

![Published and exported](8-published-exported-and-imported-back.png)
Published, exported, and imported back as a draft whose export is the same file.

![Three violations](9-three-violations-at-three-fields.png)
One save with three problems shows each violation at its own field.

![Journey edit mode](10-journey-edit-mode-a-local-node.png)
A fixture journey in edit mode, with a local node added.

![Edited here](11-a-route-copied-title-edited-here.png)
A route-copied title edited in the journey is marked, with a reset to the route.

![A removal and its cascade](12-a-removal-and-its-cascade.png)
Removing a decision shows the edge it takes and the condition it clears, previewed first.

![Restored](14-restored-as-a-local-copy.png)
The removed route node restored from its tombstone as a local copy.

![Broken down](15-broken-down-by-hand.png)
A deliverable broken down by hand into two pieces.

![Dark theme](16-dark-theme.png) ![Narrow screen](17-narrow-screen.png)
The editors in the dark theme and on a narrow screen.

[main-flow.webm](main-flow.webm): a new route, a decision, a stage with a conditional
deliverable, a milestone, and the draft published.

## Known limits

- A restored node is a local copy under new keys: a removed key never returns, so an upgrade
  treats it as the journey's own.
- A kind or answer-type change made in a journey is marked but not reset by hand; the next
  upgrade's review settles it.
- Nodes are placed by the automatic layout only; nothing is dragged.
- Saved notices stack over the bottom of the node panel until dismissed.
