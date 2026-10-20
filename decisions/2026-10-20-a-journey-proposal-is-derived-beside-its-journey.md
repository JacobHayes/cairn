# A journey proposal is derived beside its journey, and every destination's after is the engine's

Question (from the proposal-layer unit): the views need the destination as it would be after a proposal, with states, dates, frontier and rank, not only a graph. Where does that after come from, and who owns its derivation in the worker?

Call:

- The after is never assembled or replayed in the page. The engine exposes `candidate(records, destination, draft, inputs) -> Candidate { mutations, unresolved, after: Result<Records, Vec<Violation>> }`, `preview_of` over it, and `destination_after(...) -> DestinationAfter`: `Journey { document }`, `Route { route, latest }`, `Deployment { deployment }` or `Gone`. The after is derived against the after deployment, so a proposal that creates an entity and assigns it is valid, and a merge moves emails and aliases as the engine does.
- For a journey that exists, the wasm local `preview` answers `{preview, after}`. The worker has a `preview(source key, request, slot)` operation that runs it over the held journey derivation, derives the after document and holds it under a slot of its own for each preview, `<journey>/proposals/<pid>#<n>` (the page numbers them), the slot key carrying the worker's load number as `generation`. It refuses with `superseded` when the source key differs from the held derivation. A preview that replaces another gets a new slot, so a read still holding the earlier preview's key is answered from that derivation or refused as missing once released, never from its successor. Releasing the proposal's handle never frees the journey's derivation, and reloading the journey never frees the proposal's.
- For a saved route, the deployment and a journey the proposal creates, the host's preview answer carries `after: DestinationAfter`, so the page shows the proposal as saved; a journey's preview follows the reviewer's unsaved edits. A candidate that breaks a rule has no after: the views draw the destination as it is, with the touched nodes marked, and the rail leads with the violations.
- `ProposalPreview.graph` is removed: the after replaces its readers.

Alternatives: a stateless union of the before and the removed nodes (no state chips, no dates, no timeline, and it can break an invariant); replaying mutations in TypeScript (wrong for merges and for entities the after introduces); a derivation per listed proposal.

What would change it: a destination with an after too large to hold beside its own derivation.
