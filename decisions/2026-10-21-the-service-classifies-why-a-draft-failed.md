# The service classifies why a draft failed, and every host answers that class

Question (from the work making what agents are told agree with itself): an upgrade, save or
re-link the engine cannot draft was classified by each host on its own. The API answered a
missing journey or route version 404 and the rest (no route followed, a version not newer, a
draft too large) 422; MCP refused every one, so an agent was told a missing journey was a
refusal; the in-browser host reported the caller-fixable ones as a failure of the host itself.
Where is the classification made?

Call: once, in the service. `ProposeError` is `Missing`, `CannotDraft` or `Write`, decided
when the engine's draft error arrives. The API answers them 404, 422 `cannot_draft` and the
write's own answer; MCP not found, refused and the write's own; the in-browser host missing, a
new `refused` kind (caller-fixable, never a host failure) and the write's own. The draft
error's words now say what is missing ("no journey ...") rather than that it was not loaded.

Alternatives: a classification method each host matches on (the variants are the same thing
with less to get wrong); the in-browser host answering a fake HTTP status (it answers kinds,
not statuses, everywhere else).

What would change it: a fourth outcome a host must tell apart, which becomes a variant here.
