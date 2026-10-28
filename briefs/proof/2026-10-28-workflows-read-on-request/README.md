# Proof: workflows are read on request

Any agent can now read one of Cairn's nine workflows by name with `get_workflow`, including
a client that does not show MCP prompts. The in-app assistant no longer carries every
workflow's text on each request: its system prompt lists each workflow's name and purpose
and says to read one before a job it covers, and it is the same text on every request.

## Measured

In o200k tokens.

| | Before | After |
|---|---:|---:|
| Workflow text in the assistant's system prompt | 4,074 (all nine in full) | about 240 (a line each) |
| Tools listed over MCP | 20 | 21 |
| Listed tokens | 9,839 | 10,207 |

## Observed on the demo

| Call | Result |
|---|---|
| `tools/list` | `get_workflow` lists the nine names, each with its purpose |
| `get_workflow` for `walk-decisions` | its name, purpose and full text, the same text its MCP prompt serves |
| `get_workflow` for a name that is not a workflow | refused at `name` |

## Known limits

- An agent pays one call, and the workflow's tokens, the first time a job needs it.
