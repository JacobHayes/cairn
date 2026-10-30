# An answered decision or a reached milestone is not work started beneath a container

Question: a container whose only waits are its own children shows as active once work beneath it has started. Which descendants count as started?

Call: a deliverable or action beneath it, at any depth, stored `active` or `done`. An answered decision or a reached milestone opens or closes gates, and a skipped node did no work, so none of them make a container active. Done work counts, so a group mid-way through its actions stays active. One engine function holds the rule, and it is derived on every read, so stopping work takes the container back to what its waits say.

Alternatives: counting decided and reached nodes (every group reads active before anyone works, and stopping work never returns it to ready); counting only active children (a group between two steps would read ready, hiding that it is mid-way).

What would change it: feedback that a group with only answered decisions should read as underway.
