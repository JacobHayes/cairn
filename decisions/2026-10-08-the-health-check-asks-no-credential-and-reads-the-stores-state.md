# The health check asks no credential and reads the store's state

Question: a proxy checks Cairn's health with an anonymous request, and every other endpoint is behind the auth layer. What does the check answer, to whom, and what does it read?

Call: `GET /healthz` is the one endpoint outside the auth layer, still inside the `Host` allowlist, the request limits, and request observation. It answers 200 while the store answers and 503 once the store has failed closed, which only a restart clears. It reads the store's failed-closed state synchronously and touches no storage, so a probe costs nothing and never queues behind the connection pool.

Alternatives: a trivial read through the pool (it waits on the pool when the server is busy, so a loaded server looks down); keeping it behind auth with an agent token in the probe (a credential for a check that reveals nothing); exempting it from the `Host` allowlist (only a probe sending another name needs that).

What would change it: a store failure the failed-closed state does not record, or a probe that cannot send the public name.
