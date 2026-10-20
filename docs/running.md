# Running Cairn

How to run your own Cairn: the binary, its configuration, and how to put it in front of
people. For trying it on your own machine, see the [README](../README.md#try-it).

## The binary

`cairn` is one binary: `cairn serve` runs the HTTP API, the MCP endpoint at `/api/mcp`,
the revision stream, the assistant when one is configured, and the web UI on one port, over
one database file. The API, MCP, metrics, and sign-in live under `/api/`; OAuth metadata
under `/.well-known/` and the health check at `/healthz` sit beside it; every other path is
the web UI, so a screen's address (`/journeys/<id>`) loads directly. Build it with
`mise run build` (the web build is embedded; a binary built without it refuses to serve);
set `CAIRN_VERSION=vX.Y.Z` (or `$(git describe --tags)`) to stamp the version `cairn version`
reports, which is `dev` otherwise. Or download a release's `cairn-<target>` binary and its `.sha256` from the GitHub releases
(Linux, x86_64 and aarch64, glibc; `chmod +x` it and check it with `sha256sum -c`).
`releases/latest/download/cairn-x86_64-unknown-linux-gnu` is the newest stable release.
The other commands: `cairn migrate` creates the database or brings its schema up to date,
`cairn config check` validates a configuration (providers included) without opening
anything, and `cairn version`. `cairn demo` serves sample journeys from memory with no
configuration, for trying Cairn out, never for a deployment.

## Configuration

The configuration is a TOML file, passed as `--config FILE` or named by `CAIRN_CONFIG`.
Nothing a deployment must choose has a default; `cairn.dev.toml` is a complete example:

```toml
database = "/var/lib/cairn/cairn.db"     # relative paths are relative to this file
listen = "127.0.0.1:8080"
public_url = "https://cairn.example.com/" # links, OAuth redirects, and the Host served
timezone = "America/New_York"            # what "today" is, for everyone

[rank]                                   # optional: the PRD's defaults otherwise
urgency = 0.40
late = 0.15
gravity = 0.25
unlocks = 0.20
horizon_days = 14
undecided_discount = 0.5
other_owner_factor = 2.0

[[auth]]                                 # one or more, asked in this order
kind = "oidc"                            # or dev, builtin_oauth, tailscale, gcp_iap
name = "sso"
issuer = "https://login.example.com"
client_id = "cairn"
auto_link = true

[[auth]]
kind = "builtin_oauth"                   # MCP clients and agent tokens
name = "agents"
sign_in_with = "sso"

[assistant]                              # optional: the in-app assistant
protocol = "anthropic_messages"          # or openai_responses, chat_completions
endpoint = "https://api.anthropic.com/v1"
model = "a-model-name"
```

Each provider kind's settings: `dev` (`user`, `verified_emails`, which must hold at least one email,
`token`, `auto_link`,
`allow_off_loopback`), `oidc` (`issuer`, `client_id`, `client_secret`, `auto_link`),
`builtin_oauth` (`sign_in_with`), `tailscale` (`mode` direct with `socket`, or proxy, with
`trusted_proxies` listing the addresses or networks of a proxy on another machine, or left
out for `tailscale serve` on this one; `auto_link`), `gcp_iap` (`audience`, the
IAP-protected backend service as `/projects/<number>/global/backendServices/<id>`;
`auto_link`; and, for MCP clients, `client_id`, the Google OAuth client they sign in
through). Behind Google Cloud Identity-Aware Proxy, `gcp_iap` verifies the signed
assertion IAP adds to every request (`x-goog-iap-jwt-assertion`) against Google's keys, so
each person is signed in as their Google account with no sign-in step of Cairn's own.

The environment overrides `database`, `listen`, `public_url`, and `timezone` with
`CAIRN_DATABASE`, `CAIRN_LISTEN`, `CAIRN_PUBLIC_URL`, and `CAIRN_TIMEZONE`, and supplies
secrets: `CAIRN_ASSISTANT_API_KEY`, and `CAIRN_AUTH_<NAME>_TOKEN` (dev) or
`CAIRN_AUTH_<NAME>_CLIENT_SECRET` (OIDC, and `gcp_iap` with a `client_id`), `<NAME>` being
the provider's name in capitals with `-` as `_`.

## MCP clients behind Google Cloud IAP

Behind IAP, an MCP client signs in with Google and calls Cairn with the Google ID token it
gets, which IAP accepts like a browser's session. Cairn relays the sign-in and stores
nothing for it; IAP still decides who reaches Cairn on every call.

1. Make an OAuth client of type Web application in the deployment's own Google Cloud
   organization (IAP allowlists only those), with the redirect URI
   `<public_url>api/oauth/callback`, and set its consent screen to Internal (an External
   one in testing ends refresh tokens after 7 days).
2. Add the client to the backend service's IAP programmatic access allowlist.
3. Give the `gcp_iap` provider the client: `client_id` in the file and the secret in
   `CAIRN_AUTH_<NAME>_CLIENT_SECRET`. A `builtin_oauth` provider cannot sit beside it: both
   serve the same OAuth paths, and Cairn refuses a configuration with both.
4. Route these paths to a second backend service for the same instance group, without
   IAP: `/.well-known/oauth-protected-resource`, `/.well-known/oauth-protected-resource/*`,
   `/.well-known/oauth-authorization-server`, `/api/oauth/register`, and
   `/api/oauth/token`. They return no Cairn data and change nothing. Everything else,
   including `/api/oauth/authorize` and `/api/oauth/callback`, stays behind IAP.

Each person then adds Cairn once and signs in with Google in the browser:

```sh
claude mcp add --transport http --scope user cairn https://<host>/api/mcp
claude mcp login cairn --no-browser   # on a machine without a browser: open the URL
                                      # elsewhere, then paste back where it lands
```

The client refreshes the hour-long ID token by itself for as long as Google's grant lasts.
Only clients on the person's own machine can sign in this way (their redirects go to
loopback); cloud connectors cannot.

## Behind a proxy

Cairn serves plain HTTP. TLS termination, process supervision, and containers are left to
what runs it: put it behind a proxy that terminates TLS and forwards the `Host` header, and
set `public_url` to the address people use. Requests naming any other host are refused
(421), as are loopback names unless the listener is bound to loopback. `GET /healthz` needs
no credential, for the proxy's health check: it answers 200 while the server is serving and
503 once its database has failed closed, when the process also exits non-zero for its
supervisor to restart. Logs are JSON lines on standard error; metrics are at `/api/metrics`
in Prometheus's format, behind the same auth as the API (a scraper uses an agent token). An OIDC provider's redirect URI is
`<public_url>api/auth/<name>/callback`.

## Upgrades and backups

A process that opens the database holds it exclusively, so `cairn serve` and `cairn migrate`
never share one: run against a live server, `migrate` fails with a locking error. The server
applies any migrations the database lacks when it starts. To upgrade, stop the server, run
`cairn migrate` with the new binary (a failed migration then shows before anything serves),
and start it again.

There is no online backup: stop the server, copy the database file together with the two
files the store keeps beside it, its log and its write-ahead log, and start it again;
restore all three together. For `cairn.db` those are `cairn.db-log` and `cairn.db-wal` (the
log takes the database's name with its extension set to `.db-log`, so `data.sqlite` keeps
`data.db-log` and `data.sqlite-wal`). Committed writes sit in the log until the store
checkpoints them into the database file, so that file alone is not a backup. Do not open
the database with another SQLite tool while Cairn runs.

