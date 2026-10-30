# Stages are plain groups

Question: should a stage (a top-level group) carry fields of its own: a milestone it starts after and one it finishes by, each with a switch for whether it gates?

Call: no. A stage is a group like any other. A stage that waits for a milestone takes a requirement on it, so everything inside inherits the wait; a stage that must finish by one takes a `due_by` before it. Every edge gates; nothing dates a stage without making its contents wait. "Not before" says "do not start until a date", and a due-by says "finish by". A skipped milestone's pin or date answer bounds nothing wherever it is named, which is how a person drops a stage's date: skip the milestone.

Alternatives: stage fields for those milestones (a second edge kind, a bar marker, a stage-only editor and stage-only wording in every explanation, for what two ordinary fields say); only a non-gating switch (a second edge kind for a case no data uses); letting a group's own `not_before` hold its contents (changes that field for every group).

What would change it: a route that needs a stage to date its contents without making them wait.
