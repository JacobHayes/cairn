# An insertion's local edits are its differences from its version

Question (from the segments work, PRD B13 and B14): how does an insertion remember what was changed here, so an upgrade keeps it, and how are the keys of copied and later-added nodes chosen?

Call: nothing is marked. An insertion's local edits are each member's differences from its node in the segment version the insertion is on, read in the graph's keys; edges to non-members are wiring, and the root's parent, id, and title are taken from the host, so neither is ever an edit. A node of that version with no member was removed here, so no tombstone is needed. Keys are a pure function, FNV-1a over the insertion's client-minted key and the segment key, and a minted key the graph already holds or retired rejects the insert. The upgrade translates the base and target versions into the graph's keys (members by their segment key, the rest by the same minting) and runs the existing three-way merge with those differences in place of markers. Route drafts, which have no state, behave exactly as journeys do, and an edit put back to the segment's value stops being one, as a converged edit does in B7.

Alternatives:

- Stored per-field markers and tombstones for members. They need state on route drafts, where none exists, and rules on every edit path, and buy nothing once the base version is known.
- Seeding keys from the patch id and position. A proposal is drafted under one patch id and applied under another, so a preview and its apply would mint different keys, and an upgrade would have no seed for a new node.

What would change it: a need to remember an edit that has been put back to the segment's value.
