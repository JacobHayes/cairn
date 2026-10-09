# A segment has one root and is wired in through it

Question (from the segments work, PRD A21 and B13): may a segment have several top-level nodes, and how does an insertion connect to the graph it joins when the client cannot know the keys the insert will mint?

Call: a segment has exactly one root (at most one while drafting), so an insertion is one subtree that can be seen, named, collapsed, skipped, and snoozed, and two insertions under one parent differ only in the root's id (suffixed when taken). Wiring rides inside `insert_segment`: each edge joins one segment node, named by its segment key, and one host node, and by default goes through the root (the root starts after some host nodes, and some come after it), so containment carries it to everything inside. `omit` names segment nodes to leave out with their subtrees, which is how a role mapped onto one the graph already fills keeps a single filling decision; the stepper fills it in, and the plain invariant rejects two. Every mapping mistake is an ordinary violation of the final candidate.

Alternatives:

- Several roots. Their ids collide on a second insertion, the insertion has nothing single to show or act on, and wiring needs one choice per root. A group root costs nothing: its weight is 0 and it has no work.
- Separate edge mutations in the same patch, with keys computed by the client. That duplicates the minting rule in every client, and one insert stops being one event.
- Omission decided by the engine. A hidden rule where an explicit, reviewable choice serves.

What would change it: a common need to insert a segment as unrelated siblings.
