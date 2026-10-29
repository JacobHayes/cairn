# The guide keeps the core, and each tool carries the rules that govern it

Question (from trimming what an agent loads): the served guide had grown to 20 KB, but Claude Code shows an agent only the first 4 KB of a server's instructions, so most rules were never seen. Where should a rule live?

Call: the guide holds only what spans tools: what Cairn is, the loop, the write rules, ids, the status words, a short statement of when to write directly and when to propose, and a pointer to `get_workflow`. It is held to 4,096 bytes by a limit and a test. Every other rule moves to the description of the tool an agent calls first for that job (state records, dates, stages and deleting on the tools that write them, segment reuse and proposals on the proposal tool, identity deletion on the identity tool), said once, and cut where the input schema already says it. The guide's tool list is gone, since the tool list says it; the check that the guide names every tool goes with it, and the check that every identifier in served text exists now covers tool descriptions too.

Alternatives: shorten the guide by prose edits alone (the rules would still be hidden past the first 4 KB); keep the rules in workflows (read only when the agent thinks to ask).

What would change it: a client that shows the whole instruction text (the limit could rise), or the proposals design changing the direct-or-propose statement.
