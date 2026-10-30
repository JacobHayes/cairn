# Nodes pending on an unanswerable decision display as conditional

Question: a condition reads a decision that is itself undecided as unanswered, so a node gated on a follow-up decision that waits on an earlier one is not relevant until the earlier one is answered, though it may well apply then. Shown as not relevant, it would be hidden by default and counted as ruled out.

Call: relevance keeps the gating rule as it is, so gravity, unlocks, blocking, and the guards follow the gating rule exactly and a pending node counts and blocks nothing. The relevance explanation names the undecided decisions a not-relevant value rests on, found by re-evaluating the conditions with those decisions read as unknown: if the value becomes undecided, the node is pending on them. Its display state is conditional, it shows with conditional nodes and never in the hidden not-relevant count, and answer effects report it as decided later, not as dropped.

Alternatives: changing gating so an undecided decision makes the conditions that read it undecided (the deeper fix for priority, but it changes the product rule, gravity, and fixture values); showing such nodes as not relevant (wrong for the person reading it).

What would change it: second-order branches mattering for priority in real routes, which would move the rule into gating.
