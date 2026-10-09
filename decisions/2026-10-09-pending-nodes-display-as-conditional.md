# Nodes pending on an unanswerable decision display as conditional

Question (from the web redesign, PRD Gating, D8): Gating reads a decision that is itself undecided as unanswered, so value operators on it are false. A node gated on a follow-up decision that waits on an earlier one is therefore `not_relevant` until the earlier one is answered, though it may well apply then. Shown as not relevant it would be hidden by default, read "doesn't apply because...", inflate the hidden count, and count as a drop in answer effects.

Call: relevance is unchanged, so gravity, leverage, blocking, and the guards follow the PRD exactly and a pending node still counts nothing and blocks nothing. `RelevanceExplanation.pending_on` lists the undecided decisions a not-relevant value rests on, found by re-evaluating the conditions with those decisions read as unknown instead of unanswered; if the value becomes `undecided`, it is pending on them. Display state says `conditional`, the secondary line names the decision and says the node is not counted in priority until then, pending nodes are shown with conditional ones and never in the not-relevant hidden count, and answer effects report them as "decided later", not as drops.

Alternatives:

- Changing Gating so an undecided decision contributes `undecided` to conditions that read it (second-order branches would block and count at the undecided discount). It is the deeper fix for priority but changes normative text, gravity, and fixture values; recorded as not adopted for now in `2026-10-09-the-redesigns-open-questions-are-settled.md`.
- Showing them as not relevant. Wrong for the person reading it.

What would change it: second-order branches mattering for priority in real routes, which would move the rule into Gating.
