# Engine assertions sit on contracts, not two per function

Question: should every engine function carry at least two assertions on average?

Call: the engine asserts its contracts where they are checkable and meaningful: every write addresses an existing graph and record; a rejected mutation writes nothing, and every write stays in its patch's domain; each mutation's precondition and the shape of its writes; each validation stage only adds violations; after apply, one event per mutation, the revision advanced by one, and every committed graph re-verified whole. Small functions (lookups, message building, table rows) carry none rather than restate their types.

Alternatives: padding each function to two (restates what the types already guarantee).

What would change it: simulation finding a bug an assertion on a smaller function would have caught.
