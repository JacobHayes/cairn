# CI keeps only the newest cache entry of each kind

Question: every lock-file or runner-toolchain change saves a whole new set of cache entries and leaves the older set, and the repository's least-recently-used eviction can then evict main's live entries. How does CI keep the live set safe?

Call: a `prune-caches` job runs on main after the parts save and, for each kind of entry (one part's rust-cache entry whatever its hashes, the mise entry per platform), keeps the newest and deletes the rest, along with entries on tag refs, which main never restores. Only that job holds `actions: write`.

Alternatives: deleting by age (keeps entries nothing restores, and a quiet week empties the live set); a key per run with restore keys (every run saves a full set); shrinking the rust entry (what is left in it is dependencies a restore needs); turning off rust-cache's environment hash (it also drops the lock-file hash, so the entry would never be replaced).

What would change it: a second kind of entry that must outlive the newest, such as a release branch with its own caches.
