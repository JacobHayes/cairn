# A proposal is where a patch lands

Question (from simplifying how proposals are made): should a proposal be made by the write
that would have applied the change, rather than by a tool and endpoints of its own that take a
second argument shape for the same mutations?

Call: yes. `apply_patch`, `update_journey_state` and `POST /api/{domain}/patches` take an
optional `proposal` and `proposal_revision`. Without them the write is applied as before. With
`proposal`, its mutations go into that proposal: an id that does not exist creates it (the
note is its title), one that does is extended, and `proposal_revision` (required then) is the
editing revision the writer saw. Extending is its own engine mutation, so a resubmitted write
is answered from its receipt. `edit_proposal` only replaces, refreshes or discards. Save as
route is the whole content of a write to the route: `create_route` when it is new, then
`open_draft` from the journey; Cairn drafts the items, and such a proposal is not extended.
A proposal records one revision, so a write that names a proposal names no deployment
revision. The app offers the same choice as "Propose instead" on the node form and the
breakdown form, and its save-as-route, insertion and break-down-from-the-card flows send
patches that name a new proposal. Review is unchanged: the proposal screen shows the diff as a
layer over its destination, with apply and discard. The guide has one rule for when to propose.
Stored proposals keep their shape.

Alternatives: a draft that is a proposal from the start, or a branch of the destination (a
stronger model, with merge and rebase), were rejected as more concepts than the review needs.
Keeping `create_proposal` beside the field was rejected as two ways to say one thing. The add
palette has no "Propose instead", since a node that is not yet made cannot be opened after
the save; a proposal of a new node goes through the breakdown or the guide's tools.

What would change it: agents that need to build a large proposal over many turns and read it
back as they go, which would make a draft a first-class thing to write into and read.
