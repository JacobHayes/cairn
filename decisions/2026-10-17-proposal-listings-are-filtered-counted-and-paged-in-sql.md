# Proposal listings are filtered, counted and paged in SQL

Question: should each proposal listing filter, count and page in the database, or read the open proposals and filter in Rust?

Call: in SQL. Each scope is one condition on the open proposals, the count is a `COUNT(*)` under it, and the page is that condition after the cursor, newest first by creation time then id. Only the page's rows look up their destination's current state. The memory backend keeps a plain scan, the reference the conformance suite holds the database backend to.

Alternatives: filtering in Rust (every Home load and index would read and look up every open proposal of the deployment, growing with nothing to bound it); splitting a scope's `OR` into a union of index lookups (a planner question the conformance suite cannot see).

What would change it: a measured plan that cannot use an index for a scope's `OR` at real sizes.
