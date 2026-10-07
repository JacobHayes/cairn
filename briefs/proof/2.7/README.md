# Proof for brief 2.7: Route files, upgrade, save-as-route, re-link, proposal documents

A route version now exports to a file and imports back to the same graph, byte for byte. A
running journey can be upgraded to a newer route version, keeping its local edits and
offering each disagreement as a choice, and a finished journey can be saved as a new route
and re-linked to it.

## Upgrading a finished vendor evaluation to version 2

The journey had retitled environment access and re-estimated the findings; version 2 renames
access, changes the baseline's condition, adds a sign-off, and drops the workload placeholder
the journey had broken into two local tasks. The upgrade offers the title as a conflict, keeps
the estimate as a local edit, and keeps the workload as an orphan. Applied, taking the route's
title:

| | Before | After |
|---|---|---|
| `n_access` title | Access to the test environment | Environment and data access |
| `n_access` local edits | title | none |
| `n_findings` local edits | estimate | estimate (kept) |
| `n_baseline` relevant when | comparison set is not "none" | comparison set is "prior-tool" |
| `n_signoff` | absent | from the route, to do |
| `n_workload` | from the route, done | orphaned, done |
| `n_workload_query` | local, done | local, done |
| lineage | version 1 | version 2 |

While the title conflict is open, the preview shows one unresolved item and apply waits for
it.

## Save as route and re-link

- Saved as the route `vendor-saved`, the journey leaves out the workload's two local tasks by
  default; a journey created from the published version has the original's other 25 nodes,
  equal by key.
- The original journey, with its access title changed after the save, re-links to that
  version with the title offered as a conflict; keeping the journey's value marks it as a
  local edit, and an upgrade to the same version afterwards proposes nothing.

Values from the `route_walkthrough` example over the fixtures.

## Known limits

- In a debug build an upgrade at the node limit (2,000 nodes) drafts in about 4 s, against
  about 3.8 s to apply a plain rename of the same journey; the merge itself adds about 0.3 s.
