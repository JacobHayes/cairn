# The canvas attribution is hidden, with xyflow credited and its request passed on

Question (from the web redesign, PRD Non-functional): React Flow draws a small attribution badge on the canvas. Its MIT licence requires only the copyright notice in the shipped licence, and `proOptions.hideAttribution` is the supported switch, but xyflow asks, in the badge's message, its development console warning, and its README, that hiding it go with a Pro subscription and that commercial users sponsor the project.

Call: Cairn's build hides the badge and credits xyflow on You > Licenses and in the README's "Built with" list. The deployment documentation says plainly that xyflow asks organizations that hide the badge in commercial use to subscribe to Pro or sponsor, and links both. Whether the project itself sponsors is left to the user; no decision is made here.

Alternatives: keeping the badge, restyled to Graticule's mono beside the canvas controls; sponsoring as part of this change.

What would change it: a change to xyflow's licence or request, or a budget for sponsorship.
