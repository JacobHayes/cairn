# The landing goes straight to the journey index

Question (from the report that the page jumps after loading): where does `/` go, and may it wait for data to decide?

Call: `/` reads nothing. It is a route-level redirect to the journey index (`indexPath()`), placed outside the shell, so no frame is painted for `/` and the screen and the address never change after the page has loaded. The index always has content and is what the address scheme first put at `/`; Mine is one click away in the rail. The old landing screen, which read the viewer, the index and every candidate journey's "mine" before choosing Mine or the index, is gone with the code only it used.

Alternatives: always land on Mine (a person with nothing to act on opens an empty list, and it still reads every journey before it can draw); pick by the viewer's participation (needs the viewer and the index read first, so the page would wait or jump again); keep the data-driven pick (the jump that was reported).

What would change it: evidence that most opens are by participants who want Mine first; then `/` would go to Mine, still without reading anything first.
