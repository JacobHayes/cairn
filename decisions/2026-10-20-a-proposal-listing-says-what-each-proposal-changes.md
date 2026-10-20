# A proposal listing says what each proposal changes

Question (from the proposal-layer unit): a proposal row said "Proposed by you", which tells a reader nothing about the proposal, and counts of choices appeared in several places. What does a row say, and where does it come from?

Call:

- `ProposalOutline::of(&[Mutation])` names what a proposal does, from the mutations alone: header facts first (creates, renames, status, deletes, retires or restores, opens, discards or publishes a draft, upgrades to a version, re-links, inserts a segment), then nodes added, changed and removed (distinct keys, a node added and then changed counting once as added, state mutations counting as changes), requirements, entities created, edited and merged, and one count for other kinds of change. It lives in the schema crate beside `Patch::touched` because the store crates, which compute the listing's outline, do not depend on the engine. The store's listing carries `outline`, computed from the row's mutations on every read and never stored; the preview carries it too, so the rail's sentence follows unsaved edits.
- One wording function turns it into a sentence ("Upgrades to version 2 and adds 1 node"). A row's second line is that sentence, then "Needs a refresh" in warning ink; who proposed it is in the rail; no row wears an OPEN chip, and a status shows only when the proposal is not open.
- The counts are the nodes the proposal names. The preview's cascaded removals belong to the rail's Changes fold, listed under their cause, so the graph's ghosts are explained without a second, disagreeing total. The number of unresolved choices appears once, on the rail's "Needs a choice" fold.
- Home's proposals to review are one folded line after the journey groups, opened to the rows.

Alternatives: an outline from a preview per row (a derivation per listed proposal on every Home load); a count of mutations ("5 changes" means nothing to a reader); keeping "Proposed by you".

What would change it: a listing that needs the cascaded effect, which would need a derivation per row.
