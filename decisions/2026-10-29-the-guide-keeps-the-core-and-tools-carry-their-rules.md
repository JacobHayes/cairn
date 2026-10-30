# The guide keeps the core, and each tool carries the rules that govern it

Question: some clients show an agent only the first 4 KB of a server's instructions, so rules past that are never seen. Where does a rule live?

Call: the guide holds only what spans tools: what Cairn is, the loop, the write rules, ids, the status words, when to write directly and when to propose, and a pointer to `get_workflow`. A limit and a test hold it to what such a client shows. Every other rule lives once in the description of the tool an agent calls first for that job, and is cut where the input schema already says it. The guide does not list the tools; a test checks that every identifier in served text, tool descriptions included, exists.

Alternatives: shortening the guide by prose edits alone (rules still hidden past the cut); keeping rules in workflows (read only when an agent thinks to ask).

What would change it: clients that show the whole instruction text.
