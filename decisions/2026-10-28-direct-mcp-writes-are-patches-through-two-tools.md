# Direct MCP writes are patches, through two tools

Question (from simplifying the MCP tool surface): the server listed 33 tools, 14 of them direct writes, eleven of those single-op wrappers over `apply_patch` (answer a decision, move a node, assign, snooze and unsnooze, set a date, override, resolve a date conflict, create a journey, open and publish a draft). Agents had two ways to make each state change, and the wrappers failed on the combined moves the guide teaches: a note, ticks or a guard bypass that must ride with a completion. How should direct writes be offered?

Call: every direct MCP write is a patch, through one of two tools, each listing every op it takes:

- `update_journey_state` records what happened in one journey: answers, transitions, notes, ticks, who does a node, pins and recorded dates, snoozes, overrides, the journey's own name, owners and status, and a new identity an answer names. It refuses any other op at its path, pointing at `apply_patch`. A journey weight goes through `apply_patch`, since its value is the structure's own field list.
- `apply_patch` takes any patch on any target (a journey, a route or its draft, the deployment), state and structure alike, with flat arguments like every other write.

`manage_identity` stays for its paged merge read. Cairn's own drafts (upgrade, save as route, re-link) stay proposals. The capability list of the tool surface is read as capabilities, each still reachable, not one tool apiece. The engine, the HTTP API and the app's screens are unchanged.

The comparative run decided between keeping `update_journey_state` beside `apply_patch` and `apply_patch` alone. A model (Claude Code) did nine tasks three times each against the demo, per arm, each arm on its own build and guide: answer a decision; complete work that needs a note; complete ahead of what it waits on; answer naming a new person; start a journey from a route; author and publish a small route; remove a node whose removal lifts a snooze; assign; snooze. It ran twice: with tool schemas loaded on demand through tool search, and with every schema loaded up front. A first write counts as accepted when Cairn applied it; a task counts as clean when it was done with no write refused.

| Schemas | Arm | Tools | Listed tokens | Tasks done | Clean | First write accepted | Refusals | Schema tokens loaded per task | Input tokens per task |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| On demand | Before (wrappers) | 33 | 12,450 | 24 of 27 | 17 | 22 of 27 | 7 | 2,090 | 56,642 |
| On demand | `apply_patch` alone | 22 | 8,800 | 26 of 27 | 20 | 21 of 27 | 8 | 4,104 | 66,612 |
| On demand | Both tools | 23 | 10,043 | 25 of 27 | 21 | 23 of 27 | 5 | 2,503 | 54,926 |
| Up front | Before (wrappers) | 33 | 12,450 | 25 of 27 | 22 | 24 of 27 | 3 | all | 100,036 |
| Up front | `apply_patch` alone | 22 | 8,800 | 27 of 27 | 24 | 24 of 27 | 6 | all | 87,632 |
| Up front | Both tools | 23 | 10,051 | 27 of 27 | 24 | 24 of 27 | 3 | all | 84,333 |

With schemas on demand, both tools won on both measures the plan set: two more first writes accepted than `apply_patch` alone and 39% fewer schema tokens loaded per task. With every schema up front they tie on first writes accepted and tasks done, and both tools list 1,251 more tokens, yet a task took 4% fewer input tokens in all, from fewer calls (2.9 against 3.6) and half the refusals. So `update_journey_state` landed. Against the wrappers, the note-with-completion task went from 0 of 3 first writes accepted to 3 of 3 with schemas on demand. The misses on demand were mostly the new-person task (the model added the person and did not answer the decision), which no tool shape changed.

The run also showed every arm reading "go ahead anyway" as `mark_done` with a reason; the transition op now says it moves a node whatever it waits on and that `mark_done` is for a group, and refusing `mark_done` on work says to complete it instead.

Alternatives: one write tool (the run's other arm); three tools by purpose (atomicity breaks at the seams: a removal needs the unsnooze that names the node, and a date conflict's move rides with structural mutations); keeping wrappers for the frequent actions (two ways to do each, and the combined moves still fail); merging ops in the engine (events, migrations and the app, for about 200 tokens).

What would change it: clients that approve by argument instead of by tool name; agents misusing the restricted tool in practice; a direct write the patch model cannot express.
