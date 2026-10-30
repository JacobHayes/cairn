# Replay equality is the field-by-field comparison

Question: the product asks that replay compare rebuilt state with stored state field by field. Replay compares with one equality on the records. Is that enough?

Call: yes, because `Records` and everything under it derive `PartialEq`, so the equality compares every field, and a drift in any field fails it; the assertion's diff shows where. No record type in the engine or schema has a hand-written `PartialEq`, and none may exclude a field from equality.

Alternatives: a hand-written field walk beside the equality (a second comparison failing on exactly the same drift).

What would change it: a record type that needs a hand-written equality; the comparison would then have to name the fields it checks.
