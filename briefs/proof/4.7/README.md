# Proof for brief 4.7: The binary

Cairn now runs as one binary. A directory holding a config file and a database file is a
deployment, and `cairn serve` answers the UI, the HTTP API, MCP, and the revision stream on
one port.

## What it does

Run over a fresh directory whose database was seeded with every fixture:

- `cairn version` names the binary and its engine (`cairn 0.1.0 (engine 0.1.0)`), and
  `cairn config check` reads back the database, listen address, public URL, hosts served,
  time zone, and auth providers without opening the database or listening.
- Startup refuses a file without `timezone` and `public_url`, naming both and the
  environment variable that could supply each; it refuses rank constants that do not sum to
  one, a horizon of 0 days, or an undecided discount of 1.5, naming each key.
- One port serves the capabilities (no assistant configured, so none offered), a fixture
  journey's document and agent snapshot, the UI's page from the embedded web build, and the
  wasm module with a cache lifetime of a year.
- A request naming another host (`attacker.example`) gets 421 Misdirected Request on the
  API, MCP, and the UI alike, before auth; `localhost` is served on the loopback listener.
- A patch renaming a fixture journey through the API commits to the database file at the
  next revision.
- Logs are JSON lines on standard error; `/metrics` serves Prometheus text with request,
  derive, and commit series. SIGTERM waits for requests in flight and exits with status 0.
- A subscriber that stops reading loses its stream slot at the 15 s write stall, closed by
  the kernel (`TCP_USER_TIMEOUT`): measured freed 15.2 s after its receive window closed.
