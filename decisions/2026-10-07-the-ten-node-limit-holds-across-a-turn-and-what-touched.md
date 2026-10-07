# The ten-node limit holds across a turn, and what "touched" reaches

- Question: review round 1 found the limit held per tool call. A model splits "mark these fifteen done" into fifteen single-node calls (or is talked into it by text in the journey), and each applied. Removing notes counted no nodes. Skipping a container counted one node, though D1a skips its whole subtree. A role fill counts none.
- Call:
  - The ten-node limit is per turn. The turn keeps a ledger of the nodes its direct writes touched (by domain and node). A write that would carry the turn past ten is drafted instead, into one overflow proposal per destination, against the destination's current revision. Every later overflowing write in the turn is added to that same proposal by `edit_proposal`, so the overflow is seen whole and reported once, with the count the turn's writes would have touched. A single patch over ten is still its own proposal.
  - Touched nodes are resolved against the stored journey. A removed note or link counts the node it is on. Skipping or reopening a node counts its subtree.
  - A role fill or role clear still counts no node, by explicit reading: who holds a role is one answer, as a decision that fills the role is. Its effect on derived owners is reported as consequences. This reading is the owner's to confirm.
  - Replies are capped at `TOOL_LOOP_ITERATION_COUNT_MAX` (32) tool calls each, a borrowed limit; calls past it are refused to the model. The turn deadline is checked before every tool call.
  - A conversation runs one turn at a time; a second submission is answered 503 (busy).
  - The replayed dialogue is held to the newest 256 KiB (4 x `body_bytes_max`), and a provider answer to `request_bytes_max`. Both are borrowed or derived limits, pending sign-off as named limits.
  - A route that does not exist keeps no conversation until a turn drafts something.
  - Discarding a proposal is reported as a discard.
- Alternatives:
  - Per reply only: a model can spread the calls over replies.
  - Refusing the overflow outright: the user would lose what they asked for, rather than review it.
  - Counting a role fill as every node whose owner it moves: needs a derive per write, and contradicts how a filling answer counts.
  - A lock that queues the second turn: it would hold an in-flight slot for up to ten minutes.
- What would change it:
  - The owner reading "touch" to include derived effects. Then role fills and condition-driven relevance would be counted from a derive before and after.
  - The owner naming different values for the borrowed limits.
