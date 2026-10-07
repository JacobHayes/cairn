# `Graph::new` takes the deployment

- Question: construction validation now includes the plan check, whose constraints depend on relevance, and relevance resolves entity aliases through the deployment (E6). `Graph::new(document)` had none, so a journey could be accepted by apply and rejected on load, or the reverse, when a condition compares an aliased entity.
- Call: `Graph::new(document, &Deployment)`. `from_file` passes an empty deployment, since a route has no answers for a condition to compare. Hosts load a journey with the deployment it lives in, the same one derive takes.
- Alternatives: an empty deployment always (the mismatch above); the plan check only in the apply pipeline (the brief asks for it in construction validation too).
- What would change it: a host that must load a journey without its deployment.
