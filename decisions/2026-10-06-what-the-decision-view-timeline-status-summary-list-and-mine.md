# What the decision view, timeline, status summary, list search, and "mine" show

Question: several views admit more than one reading: which nodes an answer affects, which dates the timeline shows, what the status summary lists, what list search matches, and what "mine" filters by.

Call:
- The decision view lists each decision with its answer while in effect, its owners, the milestone it pins, the role it fills, and the nodes whose relevance its answer can change: a condition on the node or an ancestor reads it, up to the nearest force include, directly or through a decision it affects in turn.
- The timeline shows each milestone at its effective date, each other pinned node at its pin, and open work at its due date. Done work shows no due date; undated milestones are listed as undated.
- The status summary counts in-scope nodes by display state beside stored state and lists the open decisions not under a skipped container, in rank order.
- List search is a substring match over a node's title, description, notes, links, and resources, ignoring ASCII case only, as cross-journey search does.
- "Mine" means any participation kind; it never filters by node kind.

Alternatives: decision-to-decision edges through hidden work (quadratic at the limits); due dates on done work; Unicode case folding (in-journey and cross-journey search would disagree); "mine" by node kind (the views already filter by kind).

What would change it: a view asking for a different cut, which changes only these projections.
