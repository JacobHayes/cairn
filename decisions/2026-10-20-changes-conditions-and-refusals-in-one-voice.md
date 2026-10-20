# One voice for a patch's changes, a mutation, a condition and a refusal

Question (from the systematic removal of drifted duplicates in the web app): a patch's changes, a mutation, a condition and a refused write were each worded by several renderers, two of them printing choice ids, ISO dates, `due_by` and rule codes like "(F5)". Where does each live, and which wording wins?

Call: one home each.
- A patch's changes: `PatchList` in `detail/History.tsx` is the only place a patch is put in words; the proposal's Stale panel uses it (so it keeps the agent marker, the note and a capital "You", and dates read as words).
- A mutation: `mutationWords` in `proposals/words.ts`, built on `answerWords` (`status/answer.ts`), `dateWords`, `fieldWords` (`status/words.ts`) and `Names`. The shortfall fixes call it; its own move text is deleted. The chip's Recent keeps its past-tense summary (`describe`): it is a log line over titles the session holds, not a graph.
- A condition: `conditionWords(clause, names)` in `authoring/condition.ts`, with `conditionNames` for what a decision and a compared value are called (a choice by its label, yes or no, a date in words, an entity by name; never an id). A leaf reads `"Who runs testing?" is Partner`; the canvas card's "If ..." and the inspector's "applies if ..." use the same sentence, and a combined condition on the card lists the decisions it reads. A canvas line's hover for a condition edge now names only the decision it depends on: it had no entity names to say a value with.
- A refusal: `RejectionView` is the one renderer (stale with what intervened and Keep my edit when the screen can offer it, invalid with each rule, a change sent twice), with `violationWords` stripping the engine's rule code everywhere, not only on the sync chip. The one dismiss label is "Dismiss". Every refusal carries `data-testid="refused"` with `data-rejection` naming the kind.
- A write: `useWrite` in `data/react.ts` is the one hook (edit, receipt, no write under skew); the node, screen, proposal and authoring hooks are thin adapters, and the refusal field is `rejection` on all of them. A node write under version skew is now recorded as not sent like every other (it used to return without telling the chip). A rename shows its receipt and consequence warning.
- A shortfall (a chain short of days) is not an error: its callout says "Short of days" in place of "Error" (DESIGN: state is never colour alone), and its heading says what it needs ("Needs 5 days more than it has").
- A choice with no scope effect says "changes nothing else" instead of "no change".

Alternatives: make `describe` and the fixes short forms of `mutationWords` (rejected: tense and the lookup differ, and the log line would need a graph it does not hold); keep the card's "Who runs testing? = Partner" short form (rejected: a second condition format is the drift); a "Short of days" word through an inline style (rejected: a modifier class keeps it in the stylesheet).

What would change it: the engine giving a condition's sentence (the web app would read it), or a refusal that needs a kind of action the one component cannot offer through `extras`.
