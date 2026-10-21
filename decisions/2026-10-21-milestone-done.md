# A finished milestone is done

Question: the owner asked that a finished milestone be called done, not reached, everywhere
the word means that. What else follows from the word, and what happens to data stored with
the old one?

## The call

- **One finished state.** A milestone's stored state is `done`, the state deliverables and
  actions already use; the separate `reached` state is gone. A milestone's moves are the same
  as other work's: complete (pending to done), skip, reopen (back to pending). The separate
  `reach` transition and the `milestone_reached` event type are gone with it: finishing a
  milestone is a completion and its event is a node transition, like every other kind.
- **Setting.** The auto-reach flag is `auto_done` on the wire, in the route file schema, in
  the store and in code (`AutoDone`). Screens say "Marks itself done on its date"; the
  inspector sentence for a scheduled milestone says it marks itself done that day.
- **Derived flag.** The node projection's `auto_reached` flag is `done_by_date`, so the
  setting (what the author chose) and the derived fact (what the date made true) have
  different names.
- **Labels.** One label, "Mark done", for every kind and state, including a scheduled
  milestone (there is no "Mark done now" or "Confirm reached now"). The dated variant keeps
  its date field, labelled "Done on". The bypass entry for a blocked milestone is the same
  "Done anyway" every other kind has, so the separate act ids for reaching are folded into
  the done act. History says "Marked done"; the Timeline, status chip and filter say "Done".
- **A milestone's shortfall** reads "Finished N days off what its chain allows", as other
  finished work does; the milestone-only wording is removed.
- **Old data, migrated once** (migration 14). The tables: `node_states` is rebuilt without
  the old state (its rows become `done`), the milestone column is renamed, a journey's local
  edit of the flag names the new field, and the old event type becomes a node transition.
  The JSON columns (proposals, events, receipts) are rewritten by a code step that changes
  only the positions that hold an old name: a transition's name, a milestone's flag as a key
  or as a field name, and a node's stored state. Text a person wrote (titles, descriptions,
  notes, answers, choice ids and the map between choices) is never touched. The test runs the
  migration on a database frozen at the previous migration, with the expected result written
  by hand.
- **Not migrated.** The assistant's saved conversations are transcripts of what was said and
  stay as they are. Route files exported before this change name the old flag and no longer
  import; they are re-exported (no alias is kept).
- Earlier briefs, decision records and proofs keep their own words: they record what was
  true when they landed.

## Review calls

- The banner after the final milestone is done says so ("The final milestone is done.
  N items still in scope.") and claims everything is done only when nothing remains.
- A milestone's completion date sits under a visible "Done on" label, above the one
  "Mark done" button.
- When the final milestone is done today, the Today line is drawn alone (dashed, accent);
  the solid end line would hide it.
- A milestone's authored description (for example "its date comes from the up-front date
  decision") is the author's text and is never rewritten, so it stays as written after the
  milestone is done.
- A panel head's count stays as it is. The design document settles it: a panel's heading is
  the section title with its count in small mono beside it ("Needs attention 7"), and its
  rule against a bare number is about a number with no word near it ("Unlocks 2", not "2").
  A count that follows the word naming what is counted is that form. The critique that asked
  for "7 items" read the rule without the panel-head passage; no panel head changes.
- A scheduled milestone has no Mark done on its list row: the list offers a row action only
  to the frontier, and a milestone that marks itself done on a date ahead is not on it. The
  inspector offers Mark done on it, with the same one label, as the way to finish it early.
- When a write's only effect on the inspector is its warning (a milestone done, with the
  later things it left short or overdue), the warning sits under the sentence in the warning
  colour. It had been squeezing the sentence beside the overflow menu into a column of single
  letters on a phone, and was drawn in the success colour.

- On a phone, a late row's due line is left out when the fact under its title already says
  the same words. An empty Done on date says what to choose.
- Editing a done milestone's date offers Save only for a different date. The warning a
  write leaves stays one sentence under the sentence, as every receipt does; the nodes it
  names are on the screens, and the sentence clears when the next write starts.
- Looks that were raised and left as they are, each by what the design documents say: a
  row's one reason under its title next to the Due column (the frontier's rows are specified
  with both); a phone list's status chip wrapping onto two lines and the timeline's narrow
  label ending in an ellipsis (the table and the no-spill rules give titles the room, and the
  full words are in the inspector); the summary's actions under a row on a touch screen
  (specified there); the filter, which holds flags and relevance switches only, so a
  finished state is never hidden by it and has no choice in it.

## Alternatives

- Keep `reached` as a milestone-only stored state and say "done" only on screen: two words
  for one fact, which is the problem the owner named.
- Keep a separate `reach` move for milestones: it existed only to give the event and the
  button their own names; with one label it is a second way to do one thing.
- Accept the old flag name on import as an alias: refused, nothing depends on it.

## What would change it

A kind of finished milestone that must be told apart from finished work in the log (for
example a report of "milestones hit") would bring a milestone-specific event back.
