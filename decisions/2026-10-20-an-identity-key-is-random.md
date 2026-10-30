# An identity key is random

**Question.** What is an identity's key made of?

**Call.** `who_` plus 16 random lowercase base32 characters, minted by the writer (the Rust mint and the web app's), never derived from a name. Renaming or merging never leaves a stale-looking key, and two people with one name never collide. Minting on the writer's side lets one patch create an identity and refer to it.

**Alternatives.** A key derived from the name (stale after a rename, collides on shared names); server-minted keys (every create needs a round trip before a form can refer to the new key).

**What would change it.** A need for human-readable keys in addresses, which would argue for a separate slug rather than a derived key.
