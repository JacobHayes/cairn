# The snapshot is an overview and `list_nodes` lists nodes

- Question: after the lean rows work, a snapshot of a 214-node journey was still 19,500 tokens, nearly all of it the page of nodes, and the in-app assistant reads a snapshot on every exchange. What does an agent's first read hold, and where do nodes come from?
- Call:
  - **The snapshot lists no nodes.** It is the answers in effect, counts by state and by display state, the top of the ranked acting frontier (the rest as keys), the open decisions by rank, the placeholders needing breakdown, the unassigned work, the shortfalls, the stalled diagnostic and whether completing the journey is suggested. A subtree still scopes it. Its depth and cursor are gone, and so is the count of listed nodes.
  - **Open decisions carry their titles.** Without the node page, a key alone does not say what a decision asks. The other key lists stay keys: the unassigned list on a large journey is hundreds of entries, and titles there cost more than the rest of the snapshot together.
  - **`list_frontier` becomes `list_nodes`.** With no filter it lists every node, in rank order, 200 to a page; filters narrow it, and `next_up` is the acting frontier. `within` and `depth` scope it to a subtree and its levels (roots at depth 1). Its rows are the lean rows, with the breadcrumb of ancestors.
  - **Two options go.** The per-viewer ranking and the stalled diagnostic left the list: the snapshot has the diagnostic, and `mine` narrows to a person's items.
  - **The HTTP API follows.** `GET /snapshot` answers the same overview. The node list it already had (`GET /nodes`) gains `depth` beside `within`, so both transports scope the same way.
  - **Rows lose the snapshot's extras.** The snapshot's node entries added blockers, other participations and the earliest start; `get_node` has all of them.
- Alternatives:
  - Keep a short node page in the snapshot: any page still scales with the journey and is paid on every read.
  - Titles on every snapshot list: the unassigned list alone was 4,200 tokens on the large journey.
  - Keep the frontier as the unfiltered answer and add a separate "all nodes" tool: two tools for one list.
- What would change it: an agent that needs blockers on a list of nodes (the row would grow, or `get_node` would be called per node); a client of the HTTP snapshot that renders its nodes.
