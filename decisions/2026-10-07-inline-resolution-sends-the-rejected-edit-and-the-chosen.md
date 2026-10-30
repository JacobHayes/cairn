# Inline resolution sends the rejected edit and the chosen move as one patch

Question: a date edit rejected for a contradictory chain offers moves that each give the chain the days it lacks. Does choosing one send the move alone or with the edit? Also, how does a milestone a decision feeds unpin, and how is a link designated as a deliverable's artifact?

Call: choosing a move sends one patch, the rejected patch's mutations followed by the move, on the current revision, so the edit is kept and the chain gets its days; a shortfall in derived state, with no rejected edit, sends the move alone. Unpinning a milestone a decision feeds reopens that decision. Designating a link as the artifact retypes it as an artifact link with everything else unchanged; undesignating makes it a reference.

Alternatives: the move alone (it resolves a chain that was never stored); a direct unpin of a fed milestone (the engine refuses it); a separate designation flag (the link's type already says it).

What would change it: a resolution that takes several mutations, or links that must keep their type through designation.
