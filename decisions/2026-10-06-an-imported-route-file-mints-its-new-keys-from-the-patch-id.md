# An imported route file mints its new keys from the patch id

Question: an import mints keys for keyless nodes, roles, kinds and resources, but the service has no randomness, and a resubmitted import must be the same patch to be answered from its receipt.

Call: a new key's body is a hash of the patch id, the key's prefix and a counter, drawn again when the graph holds it or the route retired it. The file is matched against the route's latest version, whatever version it names, so an older export cannot bring back a key a later version retired. A resubmission rebuilds the patch as it was committed; the same file under the same patch id is answered from the receipt, and other content under that id is a reused id. An import is an ordinary route patch that opens a draft, reviewed and published like any other.

Alternatives: random keys (a resubmission would differ); a counter per route (two imports racing would mint the same keys); an import as a proposal (the draft it opens already waits for publishing).

What would change it: a store-assigned key sequence.
