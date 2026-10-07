# Engine assertions sit on contracts, not two per function

- Question: PRACTICES (Bounds and assertions) asks engine functions to carry at least two assertions on average.
- Call: 2.1's engine asserts its contracts where they are checkable and meaningful (about 70 across about 190 functions): every write addresses an existing graph and record (`Records::write`); a rejected mutation writes nothing and every write stays in its patch's domain (the dispatcher); each handler's precondition and the shape of its writes; each stage only adds violations; after apply, one event per mutation, the revision advanced by one, the base revision current, and every committed graph re-verified whole. Many small functions (lookups, message building, table rows) carry none, rather than restating their types.
- Alternatives: padding each function to two assertions (restating what the types already guarantee).
- What would change it: the user holding the per-function average literally, or simulation finding a bug an assertion on a smaller function would have caught.
