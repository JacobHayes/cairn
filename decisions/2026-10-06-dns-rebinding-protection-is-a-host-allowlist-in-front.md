# DNS-rebinding protection is a Host allowlist of names in front of the whole router

Question: providers that trust the peer (the dev provider without a token, a loopback or tailnet listener) let a page whose name an attacker rebinds to the server's address drive the API and MCP as that user. A host-scoped cookie and the JSON content type do not help once the browser believes the request is same-origin.

Call: one allowlist in front of the whole router, before auth, built from the public URL's host plus loopback names on a loopback listener. It matches names, not ports, case-insensitively with a trailing dot ignored: a rebinding page can choose its port but never a name other than its own, while proxies differ in whether they forward the port. A refused request is answered 421 Misdirected Request. MCP's own Host check stays off, since the listener's covers it.

Alternatives: the MCP library's check on MCP only (the API stays open); checks inside each peer-trusting provider (each would need the public URL, and the UI's routes stay exposed); matching the port too (breaks proxies and the Vite development proxy); 403 or 400 (421 says the server does not answer for that name).

What would change it: a deployment reached under several names, which would make the allowlist a configured list.
