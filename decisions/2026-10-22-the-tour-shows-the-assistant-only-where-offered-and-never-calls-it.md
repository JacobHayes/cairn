# The tour shows the assistant only where offered, and never calls it

Question: how does the tour show what the assistant is for without claiming every deployment has one, or spending a model call on a fixed lesson?

Call: the step exists only when the host offers the assistant (the capabilities say so and the host has the transport). On the practice journey the assistant's tab is the tour's: it opens where the real tab opens and shows a plainly labelled example exchange about the practice journey, rendered by the assistant's own transcript and revealed by a button that says it is an example. The live assistant is never mounted for the practice journey, so no conversation is started and no provider is called whatever the person's assistant was doing elsewhere. The in-browser demo, which has no assistant, has no step. The example reply is composed from the same derived facts as the rest of the tour, so it says only what the engine's state says.

Alternatives: always showing the step (it advertises what is not there); a live first turn (cost, latency and variable output in a lesson); a fake assistant host in the practice session (one step from starting the live conversation lifecycle); swapping only the pane's content (leaves the live dock mounted and able to open).

What would change it: an explicit request for an opt-in live exercise after the tour, with its own scope.
