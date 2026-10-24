# History, search and level answers carry what an agent reads

- Question: work cutting the MCP context cost found history answers at 18 percent of the outputs an agent reads: every event carried its actor, time, note and the stored changes of everything it wrote, so a journey's creation alone ran to 9,000 tokens. What do `get_history`, `search` and `get_level` answer, and what does that mean for the HTTP reads they share types with?
- Call:
  - **History gives each patch once, with its events by kind and subject.** A patch carries its id, actor, time and note, which its events share; an event carries its type and subject, and its own actor when a patch that applies a proposal also changes the journey directly, and the confirming user when a proposal was applied. The stored changes come only with `deltas: true`, the one opt-in argument a read takes. The patches and their order are the same either way.
  - **Search groups its hits by where the text was found.** The journey's own name and description are flags, node titles and descriptions and notes are lists of keys, and a resource keeps its node and its own key. The first page of hits and the total are as before.
  - **A level's edges are `from`, `to` and `implicit`.** The edges an edge stands for are what the canvas draws and an agent does not read.
  - **The HTTP reads keep their full shapes.** `/history`, `/search` and `/level` serve clients that render the full projections (the stale-proposal screen and the canvas); the MCP tools map them to the agent shape, as the tool set is curated for an agent's loop rather than mirrored from HTTP.
- Alternatives:
  - Keep the stored changes on every event: they are most of a history answer, and an agent asking who changed what does not read them.
  - A `detail` argument on every read: schema on every tool, rarely used well; history alone takes one.
  - Lean shapes on the HTTP reads too: the screens that read them render the full shapes.
- What would change it: an agent that usually needs the stored changes (it would pass `deltas`, or the default would flip); a client of the HTTP reads that wants the agent shape.
