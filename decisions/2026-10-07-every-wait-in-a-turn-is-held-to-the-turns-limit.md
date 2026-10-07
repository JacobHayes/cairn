# Every wait in a turn is held to the turn's limit

- Question: review round 2 found the turn's ten minutes bounded only provider calls. A stalled store under a tool call, the target read, or the conversation load or save would hold the turn's in-flight slot and its conversation without limit, and the endpoint sits outside the API's request duration.
- Call:
  - A turn's work ends `TURN_SAVE_RESERVE` (5 s, the API's request duration, borrowed) before its ten minutes; the conversation save is held to the ten minutes themselves.
  - Reading the target and loading the conversation are held to the work deadline. Past it, the turn is refused before anything is written: 503, timed out.
  - Each tool call runs on its own task, held to the deadline. A commit the deadline overtakes completes on its task, and the turn ends `turn_timed_out` with the writes it saw.
  - A save that runs out of time leaves the writes answered and the conversation unsaved, logged.
- Alternatives:
  - Cutting the tool call's future: it could stop a commit before its announcement (H6).
  - No reserve: a turn ended by its deadline could never be saved.
- What would change it: the owner naming a different reserve.
