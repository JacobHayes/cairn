# Stored data moves to identity in one migration

Question (from the rename): how do stored databases get the new names and keys?

Call: migration 11 does it in one transaction. SQL renames the auth tables first (the domain tables had taken the auth tables' old names), then a code step rewrites JSON by typed position, never blindly: a condition value is rewritten only when its decision is an identity decision in the scope the value belongs to (its own graph, a proposal's candidate, a log as its earlier events left it), answers by their type tag, proposal mutations and items by their tags, event deltas and receipt touched sets by their record kinds. User text (a text answer or condition that happens to equal an old key, a choice that reads "entity", a reviewer's choice map) is left alone. The key map comes from every typed position, so a key only a proposal or an unresolved comparison names still gets a new key: `who_` plus base32 of the SHA-256 of the old key (checked for collisions). Violation messages in persisted proposal items are reworded. Receipts keep their content hashes, so a patch retried after the migration is still recognised; renamed payloads are intentionally incompatible with old clients. No event is written for the migration. A frozen dump made by the previous code, with an independently written expected state, tests it.

Alternatives: SQL `replace` (corrupts text); a generic JSON walker with exceptions (discovers exceptions in production); wiping (loses journeys).

What would change it: another rename of this size, which would justify a reusable position table.
