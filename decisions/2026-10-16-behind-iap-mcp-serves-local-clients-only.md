# Behind IAP, MCP serves local clients only

- Question: PRD I2 calls for a hosted remote MCP server that agents reach with no local install, cloud agents included. Behind Google Cloud IAP, the relay that signs MCP clients in through Google redirects only to loopback. Does a deployment behind IAP meet I2? From brief 9.1 (MCP sign-in behind IAP).
- Call: behind IAP, I2 holds for local MCP clients: a client on the person's own machine adds Cairn with one command and signs in with Google, with nothing to install. Cloud connectors are not served there. A connector calls from its vendor's servers, so its redirect is not loopback and it would hold the sign-in after the person leaves; serving it would need Cairn to hold Google refresh credentials on people's behalf, which the relay was built to avoid. A deployment not behind IAP serves cloud connectors through the built-in OAuth server, as before.
- Alternatives:
  - A stateful broker that keeps Google refresh tokens and issues its own tokens to connectors: Cairn would store long-lived credentials, and calls with its tokens would still need to pass IAP, which accepts only Google's.
  - Opening https redirects to the relay: a code would leave the person's machine, and the token it buys still expires within the hour with no way for a connector to refresh it beside IAP without the same broker.
- What would change it: IAP accepting a credential Cairn could issue, or a need for cloud connectors strong enough to justify Cairn holding refresh credentials.
