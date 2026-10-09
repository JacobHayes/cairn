# Graph edges follow ELK's routes and are told apart by end markers

Question (from the web redesign, PRD C1): arrows were cut off and the word "condition" could fill the whole gap between two cards. The layout threw away ELK's orthogonal sections and drew beziers that ignored the space ELK had reserved.

Call: keep ELK's sections (`edgeCoords: ROOT`) and draw them as polylines with 4px corners; widen the layer gap to 96 and tighten the node gap to 24, with network-simplex placement and model order kept for stability. No text on lines: requires is solid with an arrowhead, a condition gate dotted with a hollow diamond, a stage opening dotted with a bar, a dates-only link dotted at half strength. Implicit gates stay dotted (C1). Ink follows openness: steel while waiting, border grey once satisfied. Hovering an edge gives its sentence; clicking selects it and opens an edge card with both ends. The minimum zoom is computed from the laid-out bounds, so the whole graph still fits at the size limit (C3).

Alternatives: dashed condition edges (against C1's dotted implicit gates); a midpoint marker (crowds the gap again); a smooth-step path fallback (kept in reserve if compound sections prove fiddly).

What would change it: proof screenshots showing edges attaching to card tops or bottoms, which would add fixed west and east ports.
