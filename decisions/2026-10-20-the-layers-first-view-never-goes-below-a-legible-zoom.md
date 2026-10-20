# The layer's first view never goes below a legible zoom

Question (from the proposal layer's proof screenshots): the graph opened fitted to every changed
card, so a proposal whose changes lie far apart zoomed out until no card title could be read, put
the last card against the canvas edge, and on a phone framed the cards under the peeked sheet.
How should a review's first view be chosen?

Call: frame the changed cards in the room the inspector leaves (the canvas less its floating
controls' padding, and less what a sheet covers of its lower part), but never below 75% zoom,
where a card's 14px title is about 10px on screen. When all the changed cards fit at that zoom
they are framed together, centred, at most at 100%. When they do not, the first change in the
rail's order (what waits on a choice, then added, changed and removed nodes, then requirement
ends) is framed alone, as large as the room holds it (up to 100%, never below 75%), centred (a card taller or wider than the room has its top left at the
room's), and the rail's rows bring each of the others into view by the least move, at the zoom
the view is at, in a column as on a sheet. The first view waits for the destination after to be
derived, so it frames the cards as they will be drawn. The room, the least move and the centring
are one set of functions that the picked-node move on a phone already used.

Alternatives: keep fitting all of them (illegible, and the reason for this call); frame the
largest cluster (a choice of the code's, not an order the reader can predict from the rail);
frame the first at the floor (75%), which leaves a card at about 10px on screen with room to spare on a desktop; the floor is a limit, not the reading size.

What would change it: a reader study showing titles read at a lower zoom, or a rail that lists
changes in another order.
