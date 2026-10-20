# An identity key is random

Question (from the rename): the old keys were derived from names (`e_` plus a slug). What should an identity key be?

Call: `who_` plus 16 random lowercase base32 characters, minted by the writer (the Rust mint and the web app's `newIdentityKey()`, which takes no name). A key never derives from a name, so renaming or merging never leaves a stale-looking key, and two people with one name never collide. `who_` was chosen because `i_` already belongs to insertions and `id_` reads as a generic id.

Alternatives: server-minted keys (every create would need a round trip before the form could refer to the new key, which the riding create avoids); keeping `e_` (the letter no longer stands for anything).

What would change it: a need for human-readable keys in addresses, which would argue for a separate slug rather than a derived key.
