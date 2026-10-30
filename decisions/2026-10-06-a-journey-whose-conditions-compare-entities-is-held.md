# A journey patch that depends on identities is held to the deployment it was checked with

Question: a journey patch that names no deployment revision (a pin, say) is validated against the identities and aliases loaded with it. A merge or delete committed between that validation and the patch's commit can make a condition comparing identities true, or leave a reference dangling, so the patch would commit a graph that no longer holds the plan invariants.

Call: when the journey or route draft a patch leaves names an identity, or has a condition on an identity decision's answer, the service holds the commit to the deployment revision it loaded, whether or not the patch named one. A merge or delete in between makes the patch stale, and the writer reads again. Other patches are not held to the deployment, so identity changes elsewhere never make them stale.

Alternatives: hold every journey patch to the deployment (every identity create would make unrelated journeys' writes stale); recheck the plan in the store at commit (the store has no engine).

What would change it: another way for the deployment to reach a journey's invariants, which widens the test.
