# An identity is deleted only when nothing refers to it

## Question

The identities work (delete and list) needed a rule for removing an identity and for what
"refers to" means. Journeys already name identities in answers, role fills and participations,
conditions can compare an identity decision with a key, route drafts and published versions
name them too, and a merge leaves aliases.

## Call

- `delete_identity` removes an identity, its emails and the aliases that pointed at it, and is
  rejected (`identity_in_use`, naming each journey and route) when any graph refers to it,
  directly or through an alias. Merge is how a referred-to identity goes away.
- One typed rule decides "refers to": an identity or identity-list answer, a role fill, an
  explicit participation, or a condition comparing an identity or identity-list decision with
  the key. A text decision whose condition spells the same characters refers to nothing. The
  rule lives in `cairn-schema` and the engine and both stores use it; the Turso store keeps
  the condition part as `condition_identities` rows (rebuilt for a graph whenever its nodes
  change, backfilled by the migration), replacing a substring search over condition text.
- The commit rechecks with a new `Unreferenced` precondition, authoritative over every graph;
  the engine's check sees only the graphs the service loaded, at most 16 referencing graphs
  (`CHAIN_COUNT_PER_REJECTION_MAX`, the nearest named limit) so a rejection stays bounded.
- `merge_identities` takes a survivor and a set of merged identities, checked against one
  snapshot, one event per merge with the survivor as subject. Stored proposals that held the
  single-key form are rewritten once by a migration step.
- `delete_identity` carries no list of graphs: the list is always empty, so the precondition
  is derived from the mutation. Its history sentence names no identity, because the event's
  delta holds only removals.
- A merge still names journeys only. Route graphs resolve merged keys through aliases and
  are not versioned by a merge.
- `list_identities` exists as an MCP tool only, filtered from the deployment the service
  already serves; the app keeps reading `GET /api/deployment`. The references read is a new
  endpoint, because it spans every graph.
- Turso's lock on the deployment revision row, taken by a patch that writes an identity
  reference, now also covers node-field edits (conditions), which can add a reference.

## Alternatives

- Delete anything and dangle the references: the engine could not resolve the key, and
  history would read as someone unknown.
- A `graphs` field on the delete naming what it checked: redundant with the commit-time check.
- A service method and endpoint for `list_identities`: another public surface for a filter
  over a document clients already hold.

## What would change it

A deployment with so many referencing graphs that sixteen named in a rejection is too few to
act on, or a need to delete an identity and clear what refers to it in one step.
