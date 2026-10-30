# The snapshot is the journey's facts and counts, and `list_nodes` lists nodes

Question: what does an agent's first read of a journey hold, and where do nodes come from?

Call: the snapshot is the journey's name, description, status and owners, whether completing it is suggested, and counts: by stored state, by display state, and how many nodes are on the acting frontier, are decisions that can be answered now, need breakdown, are unassigned, have a shortfall, or are what a stalled journey waits on. It lists no node, so its size never grows with the journey. Every count is the total of a `list_nodes` filter (`next_up`, `decisions_needed`, `needs_breakdown`, `unassigned`, `shortfall`, `stalled`), sharing one predicate, so they agree by construction. `list_nodes` with no filter lists every node in rank order; `within` and `depth` scope it. `decisions_needed` is the one decision filter: open decisions nothing blocks. A decision's answer and why a node waits come from `get_node`.

Alternatives: keeping the answers in effect or a top-N of each list in the snapshot (sizes that grow with the journey, and a second way to read what a filter lists); a separate filter for blocked decisions (the `blocked` filter and parents show them).

What would change it: agents that nearly always follow the snapshot with the same `list_nodes` call, or a need to read many decisions' answers at once.
