# The tour practises on a real journey, named by a per-user record

Question: where does the journey a new person practises on live, so they can try things without harming their own work, and where does "this person has seen the tour" live?

Call: the tour makes a real journey through the ordinary write path, walks it in the ordinary screens, and deletes it with one deployment patch when the tour finishes or is skipped. It is made directly, not from a published route, so the tour leaves no entry in the Routes list and does not teach versions. Nothing is mocked. The cost: while it exists, others in the deployment can see and edit it.

The person's place in the tour is one record per user outside every domain, beside sessions and agent tokens: started, finished or dismissed, the practice journey while one exists, and when it changed. A write names the record it expects to replace and a mismatch answers the current record, so two tabs share one run and a stale ending cannot clear a new one. The record names the journey before it is created, so a journey never exists without a record naming it, and an abandoned run is offered to continue or be removed on the next visit to Home.

Alternatives: a second engine over a throwaway store in the page (a large seam through code that addresses everything by URL); a shipped route published into the deployment; hand-drawn screens (they drift from the engine); a column on the users table or a flag on an identity (mixes interface state into auth or shared data, and cannot name the journey); browser storage (offers the tour again on every device).

What would change it: per-user visibility of journeys, which would make the practice journey private, or a general per-user preferences record, into which the tour's would fold.
