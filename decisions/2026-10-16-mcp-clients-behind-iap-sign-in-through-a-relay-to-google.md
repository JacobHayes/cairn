# MCP clients behind IAP sign in through a relay to Google, local clients only

Question: IAP answers every request first and accepts only a Google-signed ID token, while an MCP client sends whatever a token endpoint returns. How does a client reach Cairn behind IAP, and which clients can?

Call: the `gcp_iap` provider relays the standard MCP OAuth handshake to Google and answers the token request with Google's ID token. Google stays the only issuer and IAP keeps deciding who reaches Cairn on every call; Cairn mints and stores nothing. Discovery, registration and token are reachable without IAP; authorize and callback stay behind it. PKCE passes through to Google. Redirects are loopback only, and a cookie binds the round trip to the browser that started it. A relay and the built-in OAuth server are never configured together.

Behind IAP only local MCP clients are served. A cloud connector calls from its vendor's servers and holds the sign-in after the person leaves, which would need Cairn to keep Google refresh credentials. Without IAP, the built-in OAuth server serves cloud connectors.

Alternatives: the built-in server on public paths (Cairn mints long-lived tokens and calls lose IAP); a broker holding refresh tokens (stores long-lived credentials); a local helper program.

What would change it: IAP accepting a credential Cairn could issue, or a need for cloud connectors strong enough to justify holding refresh credentials.
