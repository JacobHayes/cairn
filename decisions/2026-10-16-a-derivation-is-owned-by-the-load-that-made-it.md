# A derivation is owned by the load that made it

Question (from the keyed-reads work): the derive worker held one derivation per journey id, and a release named the journey. When the journey manager evicted a parked journey and a later view loaded the same journey again, the first owner's late release (its load answering after the eviction) freed the derivation the second owner had just made. Who may free a derivation?

Call: the worker numbers its loads. `load` answers the key of the derivation and a handle (`{journey, load}`), and `release(handle)` frees the derivation only when the handle's load is the one the worker holds for that journey. A load that replaces another takes the journey over, so the replaced owner's release, whenever it arrives, frees nothing. The journey manager's source keeps the handle with the value it holds and releases that handle, never the journey id; its read releases the handle itself when the derivation it made is already obsolete (evicted or stopped while the load was out).

Alternatives: keep releasing by journey and have the manager never evict while a load is out (it cannot know about a load the page has abandoned, and it would hold the eviction open indefinitely); version the derivation by revision (two loads of one revision are indistinguishable, which is exactly the case that broke); refcount the loads per journey (a count answers "how many", not "which", and a late release still frees the wrong one).

What would change it: the worker holding several derivations of one journey at once (for a comparison view), which would make the handle name a derivation rather than a take-over.
