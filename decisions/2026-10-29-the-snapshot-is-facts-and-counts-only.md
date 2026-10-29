# The snapshot is the journey's facts and counts only

Question (from the owner's "why does any of this need to be in the snapshot, vs just the metadata (counts), and have the agent call other tools to get the open decisions? Tool calls are cheap"): the snapshot still carried the answers in effect, the top of the acting frontier with the rest as keys, and the top ten of four lists of nodes needing attention, so its size grew with the journey (20 KB of JSON for 214 unassigned actions) and the in-app assistant paid for it again as counts-plus-extras on every turn. What does the first read hold?

Call:

- **The snapshot is the journey's name, description, status and owners, whether completing it is suggested, and counts.** The counts are by stored state and by display state, and how many nodes are on the acting frontier, are decisions that can be answered now, are placeholders needing breakdown, are unassigned, have a shortfall, or are what a stalled journey waits on. The revision rides beside it, as for every read. It lists no node, so its size does not grow with the journey.
- **Every count is the total of a `list_nodes` filter**: `next_up`, `open_decision`, `needs_breakdown`, `unassigned`, `shortfall` and `stalled`. Two are new, `open_decision` and `stalled`; `stalled` lists the nodes the stalled diagnostic names (gates, snoozed and scheduled nodes) in rank order. The agent reads the snapshot for the counts, then calls `list_nodes` with the filter it needs, or `get_node` for one. The counts and the filters share one predicate per filter, so they agree by construction.
- **Gone with the lists**: the answers and rationales in effect (`get_node` has a decision's answer and rationale, and `list_nodes` with `kinds` decision and `states` done lists the decided ones), the subtree scope and its parameter on the tool and the HTTP read, the frontier entry and node reference types, the limit on the snapshot's lists, and the assistant's separate essentials shape for a journey: a turn now opens with the snapshot and revision as they are.
- **A new `open_decision` filter lists the open decisions**, answerable now or not (an open decision may be answered ahead of what it waits on). `decisions_needed` stays the narrower set that can be answered now. The count of open decisions is the first's total, as the snapshot's list was.
- **The stalled diagnostic's detail** (the cause of each wait, and the all-blocked flag) stays on the HTTP next list and on node reads (`get_node` says why a node is blocked, snoozed or scheduled). The snapshot says only how many nodes it waits on.
- The guide and workflows tell agents to read the snapshot for counts, then `list_nodes` with the filter they need.

Alternatives: keep the answers in the snapshot (they grow with the number of decisions, and `get_node` has them with their rationale); keep a top-N for each list (still a size that depends on the journey, and a second way to read what a filter already lists); a separate filter for blocked open decisions (a decision that cannot be answered is not worth an overview line).

What would change it: agents that nearly always follow the snapshot with the same `list_nodes` call (then carry that part), or a need to read many decisions' answers at once (then a list filter or a field on the row, not the snapshot).

This supersedes the snapshot's lists and limits in `2026-10-24-the-snapshot-is-an-overview-and-list-nodes-lists-nodes.md` and the journey essentials in `2026-10-29-a-turn-opens-with-its-targets-essentials.md`.
