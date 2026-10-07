# Derive returns the engine's own `Derived`, growing a field per pass

- Question: the brief asks for "`Derived` and `derive` holding only passes that exist", while the schema's `Derived` (the API shape) requires every D3 value on every node (dates, gravity, rank, blocking), which 2.2 cannot compute (AGENTS, No stubs).
- Call: `cairn_engine::derive(&Graph, &DeriveInputs) -> cairn_engine::Derived`, an engine type with one private field and one accessor per pass that exists (relevance, then the effective dependency graph, effective skip, participation); 2.3 to 2.5 add theirs. Projecting it to the schema's `Derived` and `NodeDerived` happens once every field exists (2.5 or 2.6), adjusting schema types that would force placeholders. `derive` takes a validated `Graph`, so it may rely on every invariant (acyclic gates in particular); a route graph derives as a fresh journey would.
- Alternatives: filling the schema's `Derived` with placeholder values for missing passes (forbidden); `Option` fields on the schema type (an API shape that says "not computed" forever after).
- What would change it: a host needing part of derive before 2.6 (none does until 4.1).
