# Behind IAP, only Google accounts sign in, and the relay holds a few request slots of its own

**Question.** IAP can also sign in Identity Platform (external) identities, whose email comes from their own provider and may be unverified. Since a caller is matched to the identity holding their verified emails, which emails does Cairn trust? And how is the relay's public token endpoint kept from starving signed-in requests?

**Call.**
- An assertion's email is verified only when its subject is a Google account's (`accounts.google.com:`). An external identity then has no verified email and is refused, so behind IAP only Google accounts sign in.
- The relay's endpoints hold a small fixed number of requests in flight of their own and answer the rest 503 with `Retry-After`, so a flood stops new MCP sign-ins and nothing else. It needs no way to tell callers apart and no configuration.

**Alternatives.** Read the external identity's nested `email_verified` claim (parsing for identities no deployment uses); a per-client rate limit keyed on `X-Forwarded-For` (forgeable without the balancer; per-client limits belong at the edge).

**What would change it.** A deployment that signs in external or Workforce Identity Federation identities, or MCP sign-ins hitting the relay's cap.
