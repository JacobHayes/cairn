# Every wait in an assistant turn is held to the turn's limit

Question: a turn's limit must bound everything the turn waits on, not only provider calls: a stalled store under a tool call, the target read, or the conversation load or save would otherwise hold the turn's slot without limit, and the turn sits outside the request duration. Cutting waits at a deadline must not leave a write landing unreported.

Call: a turn's work ends a save reserve before its limit, and the conversation save is held to the limit itself. The target read and conversation load are held to the work deadline; past it the turn is refused before anything is written. Each tool call runs on its own task: a commit the deadline overtakes completes, and the turn waits up to half the save reserve to report it; later than that, the turn's ending says a change may still land. A save that fails or runs out of time leaves the turn's reported writes standing, and fails the turn only when it wrote nothing.

Alternatives: cutting a tool call's future (it could stop a commit before its announcement); no reserve (a timed-out turn could never be saved); waiting for the overtaken call without bound (the limit would not hold); reporting a late write afterwards (a second save racing the next turn).

What would change it: late writes landing unreported often enough to want a follow-up report.
