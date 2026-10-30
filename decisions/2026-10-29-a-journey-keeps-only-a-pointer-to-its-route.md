# A journey keeps only a pointer to its route

Question: a journey copies its route's nodes. What does it keep of the route afterwards?

Call: only the route and the version it was made from, shown on the journey. Nothing is merged back, marked as locally edited, or tied to a later version. A journey's changes reach a route by saving it as a route, a plain copy whose reviewer excludes what they do not want.

Alternatives: moving journeys onto later route versions (needs per-field local-edit markers, tombstones and orphan handling, a large body of code for a flow no one uses).

What would change it: real use where journeys must follow a route's later versions, designed from that use.
