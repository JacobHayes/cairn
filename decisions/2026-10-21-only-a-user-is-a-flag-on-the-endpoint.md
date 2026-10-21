# Only a user may call an endpoint: a flag the router and the API document read

Question (from the work making what agents are told agree with itself): which endpoints refuse
an agent acting for its user was decided in three places (minting and revoking tokens in the
accounts code, the assistant's turn in the assistant, its conversation read in the API handler)
while the API document kept its own list, matched by path. Listing one's tokens shares its path
with minting one, so the document said an agent is refused there, though it is served. Where
does the rule live?

Call: `Endpoint` carries `user_only`. The router wraps each flagged endpoint in one guard that
answers an agent `403 user_only` before the handler runs, and the API document declares the
403 from the same flag. The accounts, the assistant and the conversation handler no longer
check the caller: their only callers are these endpoints. Flagged: minting and revoking agent
tokens, and the assistant's turns and conversation reads. Listing tokens is not flagged; an
agent may read its user's tokens, as it always could.

Alternatives: keeping the checks where they were and testing that the document agrees with
them (two homes kept in step by a test); checking in both the guard and the domain code
(the same rule twice).

What would change it: a second host offering token or assistant operations (an MCP tool, say),
which would need the rule below the HTTP layer; the flag would then move to the service call.
