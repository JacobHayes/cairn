# Proof: an agent writes through two patch tools

An agent connected over MCP now records anything that happened in a journey with one tool,
`update_journey_state`, and makes every other change with `apply_patch`. A note, ticks or a
guard bypass rides in the same patch as the completion that needs it, so the moves the guide
teaches land on the first call instead of being refused and redone. The eleven single-move
tools are gone, and the tool list an agent loads is smaller.

## The tool list

Measured on the demo's `tools/list`, in o200k tokens of each tool's compact JSON.

| | Before | After |
|---|---:|---:|
| Tools | 33 | 23 |
| Direct-write tools | 14 | 4 |
| Listed tokens | 12,450 | 10,051 |

## Three tasks, before and after

Observed in the comparative run below, three tries each, with schemas loaded on demand.

| Task | Before | After |
|---|---|---|
| Complete work that needs a note | the completion was refused for the missing note every time, then redone: 4.0 calls | the note and the completion went in one write, never refused: 2.3 calls |
| Author and publish a small route | a draft opened, nodes added and published over several tools, two refusals in three tries: 4.0 calls | one patch created, built and published it: 1.0 call |
| Remove a node a snooze waits on | refused for the snooze in two of three tries, then sent with its lift | refused in one of three tries, then sent with its lift |

## The comparative run

A model in Claude Code did nine everyday tasks three times each against the demo, per arm,
each arm on its own build and guide: once loading tool schemas on demand, once with every
schema up front.

| | Before | `apply_patch` alone | Both tools (landed) |
|---|---:|---:|---:|
| On demand: tasks done | 24 of 27 | 26 of 27 | 25 of 27 |
| On demand: first write accepted | 22 of 27 | 21 of 27 | 23 of 27 |
| On demand: schema tokens loaded per task | 2,090 | 4,104 | 2,503 |
| Up front: tasks done | 25 of 27 | 27 of 27 | 27 of 27 |
| Up front: refusals | 3 | 6 | 3 |
| Up front: input tokens per task | 100,036 | 87,632 | 84,333 |

Keeping `update_journey_state` beside `apply_patch` beat `apply_patch` alone on demand on both
counts: more first writes accepted, and 39% fewer schema tokens loaded per task. With every
schema up front the two tied on first writes, and both tools took fewer calls and tokens.

## Known limits

- Asked to complete work "anyway", models still reach first for `mark_done` with a reason; the
  refusal now says to complete it, and the next call does.
- Writing a single answer loads more schema than the old one-purpose tool did (1,831 tokens
  against 1,055), the price of one shape for every state change.
