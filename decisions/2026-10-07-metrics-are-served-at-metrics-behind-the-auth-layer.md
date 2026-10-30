# Metrics are served behind the auth layer

Question: whether the metrics endpoint is public, and where derive and commit durations are timed.

Call: the binary serves Prometheus metrics at `/api/metrics` behind the auth layer, so a scraper presents an agent token like any client; the endpoint is not in the OpenAPI document. It shares the API's body and in-flight limits and its observability, since the in-flight limit is one per process. The service times each derive, and the store times each commit by outcome.

Alternatives: an unauthenticated endpoint (it discloses traffic to anyone who reaches the port); timing commits in the binary through a store wrapper (every store method would need forwarding).

What would change it: a scraper that cannot send a bearer token.
