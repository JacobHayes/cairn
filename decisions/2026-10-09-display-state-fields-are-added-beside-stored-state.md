# Display-state fields are added beside stored state, never in its place

Question (from the web redesign, PRD D8, I3): the display state needs to reach the API, OpenAPI, the TypeScript client, the snapshot, and MCP, and a deployed instance and existing agent instructions read `state`, `by_state`, and the level's `group_state` today. Rename, replace, or add?

Call: add. Node rows and level nodes gain `display_state`; the status summary and snapshot counts gain `by_display_state` beside `by_state`. Every existing field keeps its name and meaning: `state` is stored state, `by_state` counts stored states and is not deprecated (those counts stay meaningful), and `group_state` stays, equal to `display_state` on groups, marked deprecated in OpenAPI and removed only in a later breaking release with a changelog line. The same rule covers `max_child_gravity`, replaced in meaning by `peak_gravity` and kept deprecated. MCP tool descriptions and the shipped instructions say to read `display_state` for "what is this node's status" and `state` for "which transition applies". The Rust types keep `deny_unknown_fields`, so a Rust client moves with the engine version, which the existing version-skew check enforces.

Alternatives:

- Repurposing `state` to mean display state, with `stored_state` beside it: one less field, but every existing reader would silently change meaning.
- Removing `group_state` and `max_child_gravity` now: cleaner, but it breaks clients for no gain while the replacement is new.

What would change it: a breaking API release, which would remove the deprecated fields together.
