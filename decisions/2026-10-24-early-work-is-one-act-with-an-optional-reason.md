# Early work is one act with an optional reason

Question (from the unit that unified starting and finishing early): finishing ahead of what a node waits on was a guard bypass that always needed a typed reason; starting ahead was unguarded and recorded nothing. A person who went ahead on purpose met two different acts, a reason box that asked for prose the engine never reads, and a "started early" chip that was read from today's blocking rather than from what happened (start work normally, reopen its prerequisite, and it read "started early"; start it ahead, finish the prerequisite, and the label vanished). What is "early", and how is it done and recorded?

## The call

- **Early is moving ahead of what the node waits on**: an open dependency (its own, inherited, or a gating stage opening) or its own not-before date. These are exactly what the `deps_done` and `date_arrived` guards check. A container's own children are not something its start waits on, and an undecided node's condition gates are not part of it (they guard nothing).
- **Both moves are the same deliberate act.** A start from `todo` is guarded by `deps_done` (children aside) and `date_arrived`, as a completion is, and it is bypassed the same way. The app says "Start anyway" and "Done anyway". The evidence guards are never checked on a start.
- **A reason is optional on every guard bypass**, the evidence guards too. The bypass names the guards, and the engine records the specific failures it excused, who and when; prose added nothing the engine reads. Skip, mark done, keep and force include keep their required reasons: they skip work or override relevance, and are not bypasses.
- **Intent is the recorded failures, never the reason.** The node's stored bypass is the record of its latest move. A start ahead stores it; a finish ahead replaces it; a completion with no bypass in its patch clears it, and so does stop (reopen already did). So a finished node's bypass is always its finish's, and a prerequisite reopened after a normal finish still warns, while work finished ahead of a prerequisite stays done early even if that prerequisite later finishes, reopens and finishes again.
- **Early is never a shortfall and never a flag.** "Started early" is derived from the record (stored `active` with a bypass that recorded a failure) and said in the node's sentence ("Started early, ahead of A", "Done early on Oct 6, 3 days before A"), each with its reason when one was given. The chip and the canvas's "active, started early" words are gone.
- **Agents** send the bypass with the start in one `apply_patch`, as they already did to finish ahead. A plain `transition_node` start of waiting work is refused with a bypassable `guard_failed`.
- Dates do not change: a start frees nothing, so started work stays held at its finish, whether or not the start went ahead of anything. This amends the earlier done-early record: its justification "starting is never guarded" becomes "a start frees nothing", and its first known limit is resolved for new starts (the label reads the record).

No store migration: every stored bypass has a reason, which an optional field reads unchanged, and no derived date changes. Work started ahead before this change carries no record, so it reads plain "active". A binary built before this change cannot load a journey holding a bypass with no reason.

## Alternatives

- **Keep starting unguarded and have the engine write the record itself.** Agents' starts and the one-click Start would stay as they are. But the pipeline would add an override write and event that no mutation asked for, after the guards ran, and a reason for a start would still need the bypass mutation, so there would be two ways to start early. Rejected for that extra machinery.
- **Leave starts unrecorded.** Simplest, but the label keeps reading today's blocking (wrong after a reopen) and the act and the record stay split.
- **Hold an unrecorded start at its start.** A prerequisite reopened while its dependent is under way would become a shortfall no act can clear.
- **Keep reasons required for evidence bypasses.** It needs a timing and evidence split in the engine and the app, a new refusal and a recovery path for it, to protect prose nothing reads.
- **Expire a finish's acceptance when the prerequisite later finishes and reopens.** It needs completion episodes in stored state, and penalises work that never waited for the prerequisite at all.

## What would change it

Refusing agents' plain starts of waiting work proving disruptive (then an unguarded start where the engine writes the record on its own, with its extra machinery, becomes worth it); the owner wanting work under way to count against a reopened prerequisite; or wanting a reason required again for some bypass.
