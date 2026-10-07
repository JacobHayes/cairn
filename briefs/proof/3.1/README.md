# Proof for brief 3.1: Store trait, memory and Turso backends, conformance suite

Cairn can now keep what an accepted patch produced, whole or not at all, in memory or in a
Turso database file, and answer from it. Both backends behave the same way case for case.

## What it does

Over a Turso file, with change sets built by hand as the engine would produce them:

- A journey holding every kind of record (a node of every kind with every field, edges,
  participations, resources, roles, answers, pins, snoozes, overrides, local edits, a
  tombstone) is committed and loads back equal to what was written.
- Two patches against revision 1: the first lands at revision 2, the second is rejected as
  stale, naming the revision that moved and the node the first patch touched, so a client
  can tell whether it may retry on its own.
- A fault between the state rows and the events leaves nothing behind: no rows, no receipt.
  The same commit sent again with no fault lands at revision 2.
- A journey patch that creates an entity moves the deployment from revision 0 to 1; a
  second create of the same entity key is rejected.
- A hard-deleted journey's events go with it, the deployment log keeps the deletion, and a
  create at the deleted id is rejected.
- A patch that would take a journey's graph to 18,024,691 bytes, past the 16 MiB limit, is
  rejected naming `graph_bytes_max`.

## Known limits

- Turso does not check a foreign key against a concurrent transaction. The store never
  relies on it: every commit writes its domain's revision row first, so a parent's delete
  and a child's insert conflict instead.
