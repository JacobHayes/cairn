# A blank Authorization header counts as absent

Question: IAP accepts the client's Google ID token from `Authorization` and passes the request on with that header present but empty. The auth layer refuses any `Authorization` that no provider claims, so without a rule for it every call behind IAP is refused. What does an empty header mean?

Call: an `Authorization` header whose values are all empty or whitespace holds no credential and counts as absent; the session and providers decide as if it were not there. Any other unclaimed value (`Basic ...`, `Bearer` with no token, an unclaimed token), alone or beside a blank one, is still refused. The test fake of IAP blanks the header as IAP does.

Alternatives: the IAP provider claiming an empty header (the check runs before any provider and is about credentials in general); stripping the header at the load balancer (it may run before IAP reads the token, and every deployment would need it).

What would change it: a client or proxy that sends a blank `Authorization` meaning something.
