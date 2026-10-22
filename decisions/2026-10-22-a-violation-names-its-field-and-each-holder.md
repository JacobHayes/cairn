# A violation names its field and each holder

Question: the form placed violations by a table of codes in TypeScript (`BY_CODE`) and a list of codes whose related nodes count as holders (`together`) (audit of the authoring editors).

Call: the engine says where. A violation carries the field at fault (`at.field`), a rule broken together by several nodes (a duplicate id among siblings, several final milestones, several filling decisions, several feeding decisions) is reported once at each holder with the first other holder (by key) in `related`, an edge violation names the edge in `related`, and a violation found by checking the whole graph carries the position of the last mutation that touched its node. The form reads those; the two tables are deleted. One violation per holder replaces one per pair, so counts of these violations in tests changed (two siblings sharing an id are two violations).

Alternatives: relate every other holder (grows with the square of the holders); keep the code tables (a copy of what the engine knows).

What would change it: a rule that needs a field the violation cannot name would add the field to the location, not a client table.
