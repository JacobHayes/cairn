# Proof for brief 7.6: Segments, model and insert

A reusable piece of a process is now a first-class object. A segment is a route of kind
`segment`: versioned, drafted, published, retired, imported, and exported as a route is, but
never started, only inserted. It has one root, so an insertion is one subtree under a parent,
wired to the rest of the graph through that root. Nothing here has a screen yet.

## What happens

The fixture `security-review` (a group root holding *Data sensitivity*, *Threat model* owned by
`reviewer`, and *Who reviews?*, which fills `reviewer`) is published as a segment and inserted
twice into the finished hiring-loop journey.

| | Before | After the first insertion | After the second |
|---|---|---|---|
| Roots with the segment's title | none | `security-review` | `security-review`, `security-review-2` |
| Nodes copied | 0 | 4, each `from_segment` with its initial state | 3 more, with other keys |
| Role `reviewer` | not in the journey | added, filled by the new *Who reviews?* | reused by id; the second owner resolves through it |
| Frontier | `offer` | `offer`, `sensitivity`, `who-reviews` | adds the second `sensitivity` |

The second insertion has to leave *Who reviews?* out: the default mapping puts the segment's
`reviewer` on the journey's, which the first insertion's decision already fills.

| Insert | Answer |
|---|---|
| second, defaults | rejected: `several_filling_decisions` (a role has one filling decision) |
| second, `reviewer` mapped to the multi-valued panel role | rejected: `insertion_invalid` (cardinality) |
| second, edges that make the root and a group wait for each other | rejected: `dependency_cycle`, nothing written |
| second, *Who reviews?* left out | accepted |
| against a stale journey revision | rejected as stale |

Around it: a segment draft with a `final` milestone, a default owner, or a second root is
rejected with paths, and publishing an empty one is rejected (`segment_rule`); a journey is never
started from a segment (`not_a_process_route`). A segment inserted into a route draft is
published with the route, and a journey started from that version holds the nodes as the
route's own, with no insertion. Removing every member of an insertion removes it. A segment
exports with `kind: segment`; a route holding an insertion exports its nodes as plain nodes.

Reads: `GET /api/routes?kind=segment` lists only segments, and `GET /api/routes/{id}/versions`
lists, under each version, where it is inserted (host, root title, whether a later version
exists). `list_routes` takes `kind`, and the agent guide has a "Reuse a segment" section and a
worked example. A database from before this keeps its routes (kind `process`) and journeys.

## Known limits

- Upgrading an insertion, saving part of a graph as a segment, and linking an existing piece to
  a segment are later; so are the screens and dedicated agent tools.
- An insertion is proposed through `create_proposal`; the web host lists processes only until
  the screens exist.
