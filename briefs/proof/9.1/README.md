# Proof for brief 9.1: MCP sign-in behind IAP

A person behind IAP adds Cairn to Claude Code with one command and one Google sign-in;
every MCP call still passes IAP.

## A local client signing in, before and after

Observed against the test doubles: a fake IAP in front of Cairn (a bare 401 without a
credential, Google ID tokens accepted only for the allowlisted client, an assertion added to
every request it lets through) and a fake Google that signs in, exchanges codes with PKCE,
and refreshes. Before is main with the same `gcp_iap` provider; after is the provider given
a Google OAuth client.

| Step the client takes | Before | After |
|---|---|---|
| `POST /api/mcp` with no credential | 401 from IAP, no challenge | the same |
| `GET /.well-known/oauth-protected-resource/api/mcp` | 404 | 200, naming Cairn as the authorization server |
| Register with Claude Code's body (both grant types, a loopback redirect) | 404 | 201, a client id; nothing stored |
| Authorize in the browser | 404 | 303 to Google with the client's PKCE challenge; Google's answer comes back through Cairn to the loopback redirect with the client's state and `iss` = Cairn |
| Exchange the code | 404 | 200, `no-store`: Google's ID token as `access_token`, and Google's refresh token |
| `tools/list` with that bearer, through IAP | 401 `invalid_token` | 200, the tool list; a journey created is recorded as the person who signed in at Google |
| An hour later | - | IAP refuses the old token; the refresh grant returns a new ID token, and MCP answers again |

What Cairn refuses:

- A bearer shaped as a Google ID token with no assertion, or beside an assertion signed by
  another key, is a 401; it never falls through to another provider.
- A callback whose state was altered, or that arrives without the cookie of the attempt that
  started it, gets a 400 page and no redirect; a client redirect that is not loopback does
  not register.
- A token answer from Google without an ID token is a 502, never Google's access token; a
  `resource` other than Cairn's is refused before Google is asked, and Google never sees it.
- A request whose body trickles in, or a Google that never answers, ends at 10 s with a
  server error. Across a whole sign-in, refresh and failed exchange, no secret, code,
  verifier or token appears in the logs at any level.
- A configuration with the relay and `builtin_oauth` together does not start, naming the
  second provider.

## Known limits

- The live check on the reference deployment (`claude mcp add`, one sign-in, tools listed,
  a refresh after an hour) is the owner's, once the deployment's load balancer and Google
  client exist; Google's behaviour is checked here only against the fake.
- Cloud connectors are not served behind IAP; only clients on the person's own machine.
- One sign-in in flight per browser: starting a second makes the first one's callback fail.
