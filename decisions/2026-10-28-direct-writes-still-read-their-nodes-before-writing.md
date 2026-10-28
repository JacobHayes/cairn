# A direct assistant write still works out its nodes before it writes

Question (from a review of the assistant's direct-write change): with the node cap gone, the wrapper only needs the nodes a write touched to report them. Should it stop predicting them with a full apply before the write, and take them from the write's own result?

Call: not now. The prediction also answers a write that would be refused with that rejection, without running it, and gives the report its nodes, including the subtree a skip reaches and the node a removed note was on. Taking nodes from the committed write needs the service's write result, the MCP tools' output and the generated API to carry them, across four crates, and the planned segment work edits the same files.

Alternatives: report only what the write's own consequences name; carry the touched set in the write result.

What would change it: the extra apply showing up in turn latency, or the segment work landing and leaving the write result open to carry the nodes.
