# A proposal applied to a journey or route creates entities as a riding create

- Question: the bake-off fixture's proposal creates two entities. Seeded through the service, its apply was refused by the store as writing entities outside the journey's domain: the store's commit shape (`crates/store/src/backend.rs`, shared by both backends) counted an entity put as a riding create (E6: creates ride in any patch, moving the deployment revision) only in an `entity_created` event, and an applied proposal's writes all sit in its one `proposal_applied` event.
- Call: an entity put in a `proposal_applied` event of a commit to a journey or route is a riding create: it moves the deployment revision and is checked for a taken key, as a direct create is. A proposal to the deployment writes the deployment's own records, as before. A shape test covers the case.
- Alternatives: seeding the bake-off with its proposal's mutations as a domain patch (the store would keep refusing every applied proposal that creates an entity).
- What would change it: proposal mutations that may edit an entity outside the deployment, which would need the event to say which puts are creates.
