# The server applies a stale write that overlaps nothing

- Question: from the audit of rules kept in more than one place, and the owner asking
  whether a retried write can silently wipe someone else's change. A stale patch was
  answered with what intervened and what it would write, and each client (the Rust client,
  the TypeScript client asking the derive worker) decided whether to resubmit it, in a loop
  of its own; an agent over MCP or REST was never resubmitted at all. Keep the behaviour,
  decide it once.
- Call: the service decides, once, for every domain patch from every surface and both
  hosts. It applies a stale patch at the current revisions when the revisions it names moved
  forward (its domain's, and the deployment's when it names one), it applies no proposal, it
  would be accepted there, and what that application writes does not overlap what every
  commit since the revisions it names wrote (read after the load it is applied to). It
  commits that application, with a receipt for the patch as submitted, and answers `applied`
  with `over`: the revisions it moved past and what intervened. Otherwise it answers `stale`
  as before. A commit beaten by another is loaded and decided again, against the same
  original revisions and a fresh read of what intervened, up to the attempt limit. A proposal's apply
  and edits and a route import are never applied this way; a revision named inside a
  patch's mutations (the journeys a merge or delete was checked against) is never moved, so
  a merge lands beside an unrelated deployment change but not after one of its journeys
  moved. Both client loops, the worker's rebase, its wasm export and the schema's rebase
  rule are deleted.
- The limit: 8 attempts per request, the owner's choice, a named constant in the service
  and a row in the limits table. Each attempt needs another commit to land on a domain the
  patch checks between the service's load and its commit, so reaching it is a runaway, not a
  workload; a patch whose last permitted commit also loses is answered `stale` and the client reads
  again.
- What an agent must do: read again after an `over` answer and after `already_applied` (a
  lost answer may have hidden an `over`), since the receipt's revision may include changes
  it has not seen. A base revision is the writer's claim to have read that state; the server
  cannot check the claim, so the rule is served where agents read it. The web tab refetches
  at every receipt's revision on its own. Not persisting `over` on the receipt: the answer
  would still depend on an agent reading it, and reading again after a lost answer is the
  simpler rule.
- What it still does not compare: what the patch read. A write made on a view that changed
  elsewhere lands when it writes nothing anyone wrote since; guards run at apply, so no
  invariant breaks. That was already true of the client retry.
- Alternatives: keep the client loops on the one schema rule (two loops, three drivers, and
  each round compared only what intervened since its own last attempt); stop applying stale
  writes at all (every concurrent edit on a busy journey would need a person, for writes
  that cannot conflict); a content hash that ignores the revisions a patch names (simpler
  receipt match, but loosens what "the same patch" means for a proposal's apply and changes
  stored receipts); answering a would-be-invalid stale patch with its own rejection (more
  specific, but the person should see what intervened first).
- What would change it: the `applied_over` outcome count showing writes landing over
  others' changes where people then report surprise, or the multiplayer testbed finding an
  applied-over write that overwrote a record, which would make every stale write go to a
  person.
