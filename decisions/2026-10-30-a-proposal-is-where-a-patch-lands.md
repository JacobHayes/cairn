# A proposal is where a patch lands

Question: should a proposal be made by the write that would have applied the change, rather than by tools and endpoints of its own taking a second shape for the same mutations?

Call: yes. `apply_patch`, `update_journey_state` and the patches endpoint take an optional `proposal` and `proposal_revision`. Without them the write applies. With `proposal`, its mutations go into that proposal: a new id creates it (the note is its title), an existing one is extended at the editing revision the writer saw. Extending is an engine mutation, so a resubmitted write is answered from its receipt. `edit_proposal` only replaces, refreshes or discards. A proposal records one revision, so a write that names a proposal names no deployment revision. The app offers "Propose instead" on the node and breakdown forms. Review shows the proposal as a layer over its destination, with apply and discard.

Alternatives: a draft that is a proposal from the start, or a branch of the destination (merge and rebase, more concepts than review needs); a separate create tool beside the field (two ways to say one thing).

What would change it: agents that build a large proposal over many turns and read it back as they go, which would make a draft a first-class thing to write into.
