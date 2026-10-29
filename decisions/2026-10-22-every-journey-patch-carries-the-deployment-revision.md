# Every journey patch carries the deployment revision

Question: `writesIdentities` decided whether a journey patch attached the deployment revision, so the editor had to know which mutations name an identity (audit of the authoring editors).

Call: the editor always attaches the deployment revision to a journey patch, and the guess is deleted. The engine's existing stale check and the service's commit precondition are unchanged. The cost is one stale answer, for a patch drafted before an identity change; identity edits are rare.

Alternatives: have the engine say which mutations need the revision (adds an API for a saving of one round trip); keep the guess (a copy of an engine rule that drifts).

What would change it: if stale answers on the deployment revision became common, the editor would redraft against the new revision itself.
