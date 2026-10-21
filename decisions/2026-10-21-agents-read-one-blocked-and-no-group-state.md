# Agents read one "blocked", and the group state is gone

Question (from the work making what agents are told agree with itself): a snapshot's
`counts.blocked`, a node row's `blocked` and a container's `all_blocked` read the derived
blocking flag, which is true for a container that cannot finish until its children do, while
`display_state`, the blocked list filter and the served guide say a container is never blocked
by its own children. An agent reading one snapshot saw two counts of blocked work that
disagreed. Beside it, a level node still carried the deprecated `group_state`, which differs
from `display_state` on a group pending on a decision and on a snoozed group although its
description said the two were equal in meaning. Which "blocked" is served, and does the group
state stay?

Call:

- Served "blocked" is the display state, everywhere. `counts.blocked` and a row's `blocked` are
  removed: `counts.by_display_state.blocked` and a row's `display_state` already say it, so a
  second field would only be a copy. `all_blocked` keeps its name and means every relevant open
  child shows the display state blocked. The engine's blocking flag stays internal: readiness,
  dependents' gates and the hidden-prerequisites marker read it.
- `group_state` and its type are removed from the level node, the OpenAPI document and the
  generated client. No app code read it, and the display state is the state of every node.
  This settles, for the group state, the earlier call that deprecated fields wait for a
  breaking release (`2026-10-09-display-state-fields-are-added-beside-stored-state.md`):
  Cairn keeps no compatibility field nobody depends on. `max_child_gravity` is left for its
  own change, since it is a separate engine value and nothing served contradicts it.

Alternatives: redefining `counts.blocked` as the display-state count (two fields saying one
thing); renaming the flag field to "waiting on its parts" and serving both (agents would still
have to choose); keeping `group_state` until a release with a changelog (it was already
contradicting the field it was said to equal).

What would change it: an agent or client that needs "cannot finish until its children do",
which would come back as its own clearly named field.
