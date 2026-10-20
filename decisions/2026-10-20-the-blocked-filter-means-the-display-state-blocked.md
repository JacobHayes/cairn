# The blocked filter means the display state blocked

**Question.** The `blocked` list filter returned groups whose display state was ready, because it read the D3 `blocked` flag, which is true for a group that cannot finish until its children do. Which meaning does the filter have? (From the derived-values work, raised while reading the list code.)

**Call.** The filter matches the display state `blocked` (D8), the one every screen shows: a gate the node waits on is unsatisfied, and a container is never blocked by its own children. The D3 `blocked` flag and `blocked_by` are unchanged: `blocked_by` still lists a container's unfinished children, because that is what "not done yet" is made of.

**Alternatives.** Leave the flag meaning and rename the filter: the person asking for blocked work means the word on the screen. Remove the group case from the D3 flag: it feeds readiness of the container's own finish and the dependents' gates.

**What would change it.** A request for "waiting on its parts" as its own filter.
