# Proof for brief 3.2: Auth providers, users, identities, agent tokens

Every auth provider can now sign someone in and turn them into a Cairn user, and each one
refuses what it should. An MCP client can get its own agent token through OAuth, acting for
the user who allowed it.

## What it does

- Dev mode: every request from this machine is the one dev user, created on its first
  request. The dev provider will not start on a listener other machines can reach unless
  told to.
- A static token signs in; a wrong token, or a credential no provider reads, is refused
  rather than falling through to a provider that would sign every local request in.
- OIDC: the sign-in sends the browser to the issuer with PKCE, a state and a nonce; the
  callback verifies the ID token and starts a session. The identity holds the email the
  issuer marked verified, lower-cased.
- An ID token with the wrong issuer, wrong audience, an expired time, another signing key or
  another nonce is refused, and no user or session is created. A callback in a browser that
  did not start the sign-in is refused, and a state works once.
- An MCP client goes from a 401 to an agent token: it finds the authorization server,
  registers, sends the user to sign in and allow it, and exchanges the code for a token.
- An agent token is shown once (the store keeps only its SHA-256 digest), signs requests in
  as the agent acting for its user, and stops working when revoked. Minting and revoking are
  both in the auth log.
- Tailscale: direct mode asks tailscaled who the peer is (a tagged machine signs no one in,
  and with tailscaled unreachable every request is refused); proxy mode trusts the identity
  headers only from this machine.

## Known limits

- Everything runs offline: OIDC against a stub issuer on loopback, Tailscale against a fake
  tailscaled on a unix socket.
