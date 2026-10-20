# The layer stays after Apply and draws every display state

Question (from the proposal-layer unit): when a proposal is applied or discarded, where does the reviewer go, and what does the destination page draw under the layer that its own filters would hide?

Call:
- After Apply or Discard the page re-reads and shows the same address with a band saying the proposal "was applied" or "was discarded" and a Close link. No navigation happens: the host remounts on the write, so a leave step would never run.
- Under the layer the page's display-state filter is lifted, so every node a proposal touches is drawn, including nodes that are done, skipped or not relevant.
- A timeline lists the nodes a proposal removes in a "Removed by this proposal" fold instead of ghost rows, and draws a moved date at its new place only.

Alternatives: leaving for the destination on Apply (loses the receipt of what was applied; fights the remount); keeping the page's filters (hides changed nodes); ghost rows and a hollow before-mark on the timeline (more marks than the three the layer allows).

What would change it: reviewers asking to see a moved date at both places, or a flow that wants to land elsewhere after Apply.
