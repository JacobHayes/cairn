# A drawn line opens the links it stands for

Question (from the edge-card work): a journey's line card was addressed by the two cards the
line joins and explained a link between exactly those two nodes, so a line between folded
stages or rolled-up cards, which stands for edges whose ends are inside them, opened "no such
link". The card also said every link "waits on" its requirement, including a stage opening that
holds dates only; the hover's "done" was read from the requirement's display state, which is
not the engine's rule for whether a dependency still holds (a skipped container with kept work
pending still holds; a prerequisite the answers rule out, or a condition a force include
drops, stops holding without finishing); and a route had no line card at all.

Call:

- The address of a line names the two cards it joins at the view the query describes: the
  view's own level, read plain, with no node selected and nothing unfolded. A line between two
  cards that are the link's own ends keeps its old address, so existing links work. An edge
  address is always the Graph, on a journey and on a route, whatever `view=` says.
- Only lines the view's own level draws are selectable. While a node is selected, a line that
  only the selection draws (inside a stage opened to show it, or to a card Reveal adds) keeps
  its hover and opens no card. The flag is carried on each line in the composed model, so the
  drawn canvas, its layout and the flags always belong together.
- The card is built from the graph's own edges the line stands for (the engine already lists
  them on every drawn edge), one row per edge, strongest kind first and then in the engine's
  order. Words per kind: a requirement (and, when its dependent holds nodes, that it binds
  everything inside), a condition, a stage opening, and a date-only opening, which never says
  anything waits. Each end of each row is a link, and Go to opens either card the line joins.
- State per edge comes from the dependent's own unsatisfied gates as the engine derives them
  (`blocked_by`, matched by requirement and origin), not from display state. It exists only
  where the line carries state, so a route's card says none. The hover chip reads the same
  value and says "still waiting" or "holds nothing back now" instead of "done".
- One card component, rendered by the shared canvas path into the inspector from the line the
  canvas draws, serves the journey and the route; a line the view no longer draws says so and
  offers both ends that are nodes of the graph. A card lists its first 50 links and offers the
  rest: a line between two stages at the node limit can stand for tens of thousands of links
  (measured with 2,000 nodes and 32,000 requirements between two stages: building the lines
  took 41 ms; rendering every row took 1.6 s, the first 50 under 20 ms).

Alternatives: an address naming the underlying edges (a set in a path, unbounded for a folded
stage); a view-independent rule, every edge from inside one card's node to inside the other's
(it over-counts edges into a container's drawn children, which have lines of their own); the
frame reading the level again to find the line (a second copy of the canvas's level request and
side model); widening the engine's level query so an edge address draws both ends as a node
address draws its node, which would keep a selection's own lines selectable (a change to the
query the API and agents also read, for lines a selection only shows in passing); state from
display state (it disagrees with blocking in the cases above).

What would change it: the level query taking several drawn nodes for another reason, which
makes the widening free; the owner wanting a selection's own lines selectable.
