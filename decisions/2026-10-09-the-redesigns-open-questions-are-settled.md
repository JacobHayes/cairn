# The redesign's open questions: leverage keeps its name, conditional nodes show, the badge is hidden, rank and Gating unchanged for now

Question (from the web redesign): five questions were left open for the user. (1) Should rank normalize gravity and leverage over the acting frontier rather than every open node? (2) Rename leverage everywhere, or only in the UI? (3) Hide conditional nodes by default like not-relevant ones? (4) Should a decision that is itself undecided make the nodes it gates undecided rather than not relevant? (5) Should the project sponsor xyflow, given the app hides its attribution?

Call:

- (1) Not now. The rank formula and fixture values stay as the PRD has them. A blocked node, often a milestone before the final one, can hold the journey's largest leverage and shrink every actionable node's normalized share, so leverage barely moves rank; normalizing over the frontier (snoozed included, so snoozing does not reshuffle) is the likely fix, but it changes normative Priority text and fixtures, so it waits for sign-off.
- (2) The UI says "Unblocks"; the PRD, API, and MCP keep `leverage`, and the glossary notes the UI name.
- (3) Conditional nodes are shown, ghosted, and rolled up at the Stages step; only settled not-relevant nodes are hidden by default (`2026-10-09-not-relevant-nodes-are-hidden-by-default.md`).
- (4) Not now. Relevance is unchanged and pending nodes display as conditional (`2026-10-09-pending-nodes-display-as-conditional.md`).
- (5) The badge is hidden, xyflow is credited, and its request is passed on (`2026-10-09-the-canvas-attribution-is-hidden-and-xyflow-credited.md`). No sponsorship decision is made here.

Alternatives: for (1), defining leverage only for frontier nodes as well, which adds little once blocked nodes show no Unblocks figure; for (2), renaming the concept on the wire; for (3), hiding conditional nodes with a count, which would make the plan look smaller than it may be; for (5), keeping a restyled badge.

What would change it: for (1), the user's sign-off on the frontier normalization, landed as a follow-up to the priority-explanations brief with new fixture values; for (2), agents repeatedly echoing "leverage" to people; for (4), second-order branches mattering for priority in real routes; for (5), a project budget for sponsorship.
