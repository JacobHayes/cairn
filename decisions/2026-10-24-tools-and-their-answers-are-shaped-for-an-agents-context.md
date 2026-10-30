# Tools and their answers are shaped for an agent's context

Question: an agent loads the tool list on connecting and reads every answer into its context, and the in-app assistant pays the same cost on every exchange. How do the list and the answers stay small without losing what an agent needs to act correctly, given that the HTTP API shares the types?

Call:

- The list is compacted where it is built, on a copy of each tool's generated input schema: it drops what an agent cannot act on (titles, defaults that say nothing, formats, length and count bounds the server enforces anyway) and keeps value ranges and an id's pattern. The doc comments stay whole for the OpenAPI document. No output schemas are listed.
- Answers carry what the agent acts on, once. A list row is the node's key, title, kind, both states, parent or breadcrumb, owners, dates, the flags that hold, and its rank as one rounded number; a settled node drops slack and rank. A container's blockers leave out its own children. History gives each patch once with its events by kind and subject, and stored changes only with `deltas: true`. Search groups hits by where the text was found. A level's edges are `from`, `to` and `implicit`. A write answers its status and revision, plus what it caused only when there is something.
- The HTTP reads keep the full shapes the screens render from; the MCP tools map them to the agent shape. The snapshot is the agent's read on both transports.
- Budgets are limits: the tool list, one tool's definition, the mutation sketch and the bytes per node row, measured as compact JSON, with tests that fail past them.

Alternatives: lean shapes on every HTTP read (the screens render the full ones); a `detail` argument on every read (schema on every tool, rarely used well); merging overlapping tools (little saved, and one tool doing two things); a real tokenizer in the budget tests (a large dependency for a ratio bytes already track).

What would change it: clients that load tool schemas only on use (each tool's size would matter more than the list's), or an agent that usually needs what a lean shape leaves out.
