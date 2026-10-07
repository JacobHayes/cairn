# Proof for brief 4.9: HTTP API completion: projections, proposals, route files

Every operation the service offers is now an HTTP endpoint: every projection of a journey,
node detail with capped and pageable explanations, proposals reviewed and applied, upgrades,
saves as route and re-links drafted as proposals, and route files exported and imported.

## What it does

Observed over the vendor evaluation after kickoff, read by Ann on 2026-10-06:

- Every projection (next list, snapshot, list, node detail, level, trace, decisions,
  timeline, summary, mine) carries the journey revision and the today it was derived for.
  The next list ranks `n_access`, `n_decision_meeting`, `n_workload`.
- Node detail caps each explanation list and the rest pages, every entry once. A root with
  103 nodes downstream:

  | Read | Gravity contributors | Next cursor |
  |---|---|---|
  | node detail | 50 of 103 | - |
  | explanation page 1 | 50 | 50 |
  | explanation page 2 | 50 | 100 |
  | explanation page 3 | 3 | none |

- An agent drafts a proposal and edits it to revision 2 while the journey stays at revision
  3. Bob previews it, applies the revision he reviewed (an older one is refused as stale),
  and every event of the apply records him as the confirming user. Created again after a
  lost response, the proposal is fetched, not duplicated.
- When the journey moves after a proposal was drafted, its apply is refused with what
  intervened; refreshed against the new revision and previewed again, it applies.
- The upgrade to version 2 is drafted as a proposal with a conflict, kept local edits, and
  an orphan; the journey stays on version 1, the apply is refused until the conflict is
  resolved, then the journey follows version 2.
- Version 1 exported and imported back opens a draft whose export is the same file.
