# Mutation semantics the requirements leave implicit

Question: the requirements define what can change but not every mutation's shape.

Call: a mutation carries only what its author chose, never a value the server must trust or a consequence the engine can work out.

- Transitions carry no dates: the engine records today as a start, finish or decided date, and a recorded date is changed by its own mutation. Reopening clears the node's recorded dates.
- A node whose kind changes starts over in the new kind's initial state, since its old machine does not apply. An answer-type change keeps the answer, so it is refused unless the patch also reopens the decision.
- A removal names what it removes: anything the subtree holds that it does not name rejects it, and a name the subtree no longer holds (a child moved out since) is ignored, since removing less than the author saw is safe.
- Removing a role drops its direct fill; any other reference to it rejects the patch.
- Applying a proposal is one event holding every write its mutations made, attributed to the proposal's author with the applying user as the confirming user.

Alternatives: dates on transitions (clients would supply today); rejecting kind changes on a node with state; rejecting a role removal while the role is filled.

What would change it: a mutation whose effect cannot be worked out from the graph it lands on.
