# A journey has two pages, Next and Plan, and one projection switcher

Question (from the web redesign, PRD C9 to C13, C16, C18): a journey had nine tabs (overview, canvas, next, list, triage, walkthrough, decisions, timeline, summary), several of them the same data drawn differently. How should a journey's surfaces be organized?

Call: two pages chosen by intent. Next is the acting frontier ("what can be done now") with list and cards projections; Plan is the whole journey with graph, list, and timeline projections. One segmented switcher at the right end of the toolbar picks the projection, in one global order, each page showing its subset and never a disabled segment, so its right edge never moves. Decisions is a toggle that means the same thing on every projection: on Next cards it is the walkthrough, on the Plan graph the decision view, on the Plan list the decision table. The overview and summary become the journey header and a journey card in the detail column when nothing is selected, which opens to a Summary page that keeps its own address for reports and printing. Every old address redirects.

Alternatives:

- Three pages (now, plan, about): "about" is read once and rarely; its lifecycle moves belong in the header and its content in the journey card.
- Scopes crossed with projections (twelve combinations): several were degenerate, such as a faded whole graph as "next".
- One toggle mixing a scope with projections: unclear what it switches.

What would change it: a third recurring intent people switch to often enough to deserve a page.
