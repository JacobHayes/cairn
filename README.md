# Cairn

Cairn structures a process as one graph of nodes that must be decided or done: decisions,
deliverables, actions, milestones, and groups. A route is the reusable template; a journey is
a live graph with answers, progress, people, and dates. Every change is a validated, atomic
patch, and Cairn ranks the frontier to answer "what should I do next?". People use it through
a web UI; agents use the same capabilities through an HTTP API and an MCP server.

Cairn is being built in feature briefs. The binary runs: `mise run serve` starts a local
deployment at http://127.0.0.1:8080/.

## Documents

- [`PRD.md`](PRD.md): what Cairn does, and its glossary.
- [`ARCHITECTURE.md`](ARCHITECTURE.md): how it is built.
- [`PRACTICES.md`](PRACTICES.md): how code is written and verified.
- [`AGENTS.md`](AGENTS.md): the entry point for implementers, then [`briefs/`](briefs/README.md).
- [`DECISIONS.md`](DECISIONS.md): judgment calls awaiting review.

## Develop

Tools are pinned in `mise.toml`; tasks live in `mise-tasks/`.

```sh
mise install          # pinned Rust toolchain, Node, wasm tool, and cargo-patina
mise run check        # every rung of the validation ladder that exists
mise run check:fast   # rungs 1 to 3: the inner loop
mise run check:1      # one rung
mise run gen          # regenerate the generated paths
mise run sim          # simulation campaigns under patina; not part of check
mise run build        # the release binary, with the web build embedded
mise run serve        # a local deployment from cairn.dev.toml, signed in as the dev user
```

## Run

`cairn` is one binary: `cairn serve` runs the HTTP API, the MCP endpoint at `/mcp`, the
revision stream, the assistant when one is configured, and the web UI on one port, over one
database file. Build it with `mise run build` (the web build is embedded; a binary built
without it refuses to serve). The other commands: `cairn migrate` creates the database or
brings its schema up to date, `cairn config check` validates a configuration (providers
included) without opening anything, and `cairn version`.

The configuration is a TOML file, passed as `--config FILE` or named by `CAIRN_CONFIG`.
Nothing a deployment must choose has a default; `cairn.dev.toml` is a complete example:

```toml
database = "/var/lib/cairn/cairn.db"     # relative paths are relative to this file
listen = "127.0.0.1:8080"
public_url = "https://cairn.example.com/" # links, OAuth redirects, and the Host served
timezone = "America/New_York"            # A9: "today" for everyone

[rank]                                   # optional: the PRD's defaults otherwise
urgency = 0.40
late = 0.15
gravity = 0.25
leverage = 0.20
horizon_days = 14
undecided_discount = 0.5
other_owner_factor = 2.0

[[auth]]                                 # one or more, asked in this order
kind = "oidc"                            # or dev, builtin_oauth, tailscale
name = "corp"
issuer = "https://login.example.com"
client_id = "cairn"
auto_link = true

[[auth]]
kind = "builtin_oauth"                   # MCP clients and agent tokens
name = "agents"
sign_in_with = "corp"

[assistant]                              # optional (I5)
protocol = "anthropic_messages"          # or openai_responses, chat_completions
endpoint = "https://api.anthropic.com/v1"
model = "a-model-name"
```

Each provider kind's settings: `dev` (`user`, `verified_emails`, `token`, `auto_link`,
`allow_off_loopback`), `oidc` (`issuer`, `client_id`, `client_secret`, `auto_link`),
`builtin_oauth` (`sign_in_with`), `tailscale` (`mode` direct with `socket`, or proxy;
`auto_link`). The environment overrides `database`, `listen`, `public_url`, and `timezone`
with `CAIRN_DATABASE`, `CAIRN_LISTEN`, `CAIRN_PUBLIC_URL`, and `CAIRN_TIMEZONE`, and supplies
secrets: `CAIRN_ASSISTANT_API_KEY`, and `CAIRN_AUTH_<NAME>_TOKEN` (dev) or
`CAIRN_AUTH_<NAME>_CLIENT_SECRET` (OIDC), `<NAME>` being the provider's name in capitals
with `-` as `_`.

Cairn serves plain HTTP. TLS termination, process supervision, and containers are left to
what runs it: put it behind a proxy that terminates TLS and forwards the `Host` header, and
set `public_url` to the address people use. Requests naming any other host are refused
(421), as are loopback names unless the listener is bound to loopback. Logs are JSON lines
on standard error; metrics are at `/metrics` in Prometheus's format, behind the same auth as
the API (a scraper uses an agent token).

CI (`.github/workflows/check.yml`) runs `mise run check` on every push and nightly, and
`mise run sim` nightly outside the gate.

## License

[Apache-2.0](LICENSE).
