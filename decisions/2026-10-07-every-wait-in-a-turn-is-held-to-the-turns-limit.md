# Every wait in a turn is held to the turn's limit, and a write the deadline overtakes is reported when it lands within half the save reserve

- Question: review round 2 found the turn's ten minutes bounded only provider calls. A stalled store under a tool call, the target read, or the conversation load or save would hold the turn's in-flight slot and its conversation without limit, and the endpoint sits outside the API's request duration. Holding each wait to the deadline then left a write the deadline overtook landing unreported, against I5's "every direct write is reported", and a failed conversation save discarded a turn's answer after its writes had landed.
- Call:
  - A turn's work ends `TURN_SAVE_RESERVE` (5 s, the API's request duration, borrowed) before its ten minutes; the conversation save is held to the ten minutes themselves.
  - Reading the target and loading the conversation are held to the work deadline. Past it, the turn is refused before anything is written: 503, timed out.
  - Each tool call runs on its own task, held to the deadline. A commit the deadline overtakes completes on its task; the turn waits for it up to half the save reserve (2.5 s of the 5 s) and reports its write if it lands, then ends `turn_timed_out` with the writes it saw. A write later still is not in the answer, and the turn's ending note says a change still being written may land after it.
  - A save that runs out of time, or fails, leaves the writes answered and the conversation unsaved, logged. A failed save fails the turn only when the turn wrote nothing.
- Alternatives:
  - Cutting the tool call's future: it could stop a commit before its announcement (H6).
  - No reserve: a turn ended by its deadline could never be saved.
  - Waiting for the overtaken call without bound: the turn's limit would no longer hold.
  - Reporting a late write in the conversation afterwards: a second save outside the turn, racing the next turn.
- What would change it: the owner naming a different reserve; a save reserve that is needed whole in practice; or a late write that lands unreported often enough to want a follow-up report.
- History: the assistant brief's second review (4.4) bounded every wait and let an overtaken commit finish unreported; the assistant panel brief (5.8) added the wait of half the reserve, the ending note, and the rule that a failed save keeps a turn's answer (first recorded as "a write the turn's deadline overtakes is reported when it lands within half the save reserve").
