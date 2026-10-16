# The journey index lives at the root

Question (from the report that the page jumps after loading, and then from asking why `/` redirects at all): where is the journey index's address, and may `/` wait for data or move the viewer?

Call: the journey index is `/`, with its filters in the query (`/?status=any`). It is the shell's index route, so nothing redirects to it and the screen and the address never change after the page has loaded. `/journeys` as a bare address does not exist (journey pages stay under `/journeys/<id>/...`), and no alias or redirect keeps it alive. The old landing screen, which read the viewer, the index and every candidate journey's "mine" before choosing Mine or the index, is gone with the code only it used. The rail marks Journeys current on `/` by exact match, so it is not current on every page.

The two redirects that remain are real: `/journeys/<id>` resolves to the journey's graph (the address agents post, the default view is a choice of the app), and a node's deep link `/journeys/<id>/nodes/<key>` resolves to the graph address that carries the reveal state. Neither papers over a wrong address.

Alternatives: a route-level redirect from `/` to `/journeys` (a second address that exists only to be left); land on Mine (a person with nothing to act on opens an empty list, and it reads every journey before it can draw); pick by the viewer's participation (the page would wait or jump again).

What would change it: evidence that most opens are by participants who want Mine first; then `/` would be Mine, still reading nothing first, and the index would need its own address.
