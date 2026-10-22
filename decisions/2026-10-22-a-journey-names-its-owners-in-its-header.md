# A journey names its owners in its header

## Question

From the journey-owners work: a journey had no owner of its own. Work belongs to identities
through participations, but nothing said who answers for a journey as a whole. Where do
owners live, who gets them, and what do they affect?

## Call

1. A journey has zero or more owners, stored in its header beside the name and description
   (a `journey_owners` table in the SQLite store, an `owners` set in the header event).
2. Owners are identities, and an owner is an identity reference like any other: the identity
   cannot be deleted while a journey names it, a merge follows the alias and must name the
   journey, `referencing` finds the journey, and "mine" counts it.
3. Owners are never defaulted: not the creator, not the graph's default owner. They are set
   by `create_journey` and `edit_journey` only.
4. `edit_journey` keeps owners when the field is absent, replaces them when given and clears
   them when given empty. This differs on purpose from the description, where absent clears:
   the owners field is new, so no client relies on absence meaning anything, and a client
   that only renames must not drop the owners.
5. An upgrade keeps owners. A saved route never carries them (they are not part of the
   structure), so save-as-route stays graph-only.
6. Owners share the limit of 100 identities per list; no new limit.
7. Owners are read through aliases and never rewritten by a merge.
8. They are shown on the journey card, the Summary's About, the index (Owner filter, and
   named on the row), the snapshot's new `journey` facts, and the `list_journeys` rows
   and filter.
9. On the index they sit on the row's context line, after the lineage, not in a column of
   their own. The table is already narrow beside the inspector, and a column would push
   out the figures people scan; the second line already holds "who and where from" facts.
   A phone row stacks them in the same order.
10. An owned journey is "mine" for its owner even with no work of theirs in it; Home does not
    list a journey for being owned.

## Alternatives

- Graph state beside role fills: owners would travel with the structure into saved routes
  and upgrades, which is not what an owner is.
- The graph's default-owner role as the journey owner: that role is for unassigned work.
- The creator as the default owner: a journey made by an agent or on someone's behalf would
  start owned by the wrong person.
- A separate `set_journey_owners` mutation: one more write for something edited with the
  name and description.
- An Owners column on the index: see 9.
- Home rows for owned journeys: owning is not work waiting; proposals for owned journeys
  already wait on the owner there.

## What would change it

Owners needing an effect on nodes (for example unassigned work going to the journey's
owner), or a model of teams.
