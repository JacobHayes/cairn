# Home shows what can move and counts the rest

Question (from the Home and indexes work): C16 said Home lists the union of each journey's "mine".
For a viewer with hundreds of owned nodes that is a wall, mostly work that cannot start yet.

Call: per active journey, Home lists the viewer's work that can move now, in that journey's rank,
bounded at five rows with Show all; the viewer's remaining "mine" in that journey is counted in
the group head and is one click away (the journey's own list, filtered). PRD C16 is reworded to
say so.

Alternatives: the full union (unbounded and unranked across journeys); only the count (nothing to
act on from Home).

Would change it: cross-journey ranking (`Later`), which could order the groups by what is most
urgent.

The journeys index reads only the rows near the view (a long table derives the journeys on screen
and releases the rest), measured with 300 journeys: rows appear in 0.7s instead of 19.8s and the
heap is 68MB instead of 148MB.
