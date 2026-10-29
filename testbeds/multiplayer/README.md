# Multiplayer testbed: many clients, one journey, under patina

On the simulation track (PRACTICES, Simulation with patina): exploratory, run by
`mise run sim`, never by `mise run check`. Cairn's real HTTP server, store, and Rust client
under the patina native shim, with clients patching one journey through network faults and
fault sites in the store, and the multiplayer invariants checked at the end of every run.

## The program

`src/main.rs` assembles a composition root as the binary's will (`src/world.rs`): the SQLite
store on a file in the run's virtual filesystem, the in-process notifier, the dev sign-in
provider (no token: every request is the dev user), and `cairn_api::router`, served by
axum on `127.0.0.1:7878`. Everything runs as tasks on a multi-thread tokio runtime with four workers, so requests
run on several threads and PCT reorders them
(`decisions/2026-10-18-the-multiplayer-testbed-runs-a-multi-thread-runtime.md`).

Setup creates journey `j_shared` with three nodes and one pair of identities. Then:

- **Clients** (`--clients`, default 4; `src/client.rs`): each drafts `--actions` patches
  (default 8), a seeded pause apart, against the journey or deployment as it last fetched
  it: a note, a rename of one of the three nodes, and, for client 0 halfway through, an identity merge (moves the deployment). Each
  patch is sent once to the server (H5: it answers a stale patch stale, and the client surfaces it), over a transport
  retry that resends a request whose answer did not arrive within 200 ms, unchanged, so a
  patch that landed is answered from its receipt. After each acknowledgement the client
  fetches the domain again: the next snapshot must hold the patch.
- **The agent** (one more client, numbered after the others): the same drafts and the same
  transport retry, calling the service in process as an agent host (MCP, the assistant) does.
  Each attempt runs on its own task, so an attempt the agent stops waiting for keeps going.
  The server runs each write on its own task too, so a write whose connection closes keeps
  going, and an HTTP resubmission can meet its original.
- **Views** (one per HTTP client; `src/view.rs`): fetch the journey and the deployment, then
  follow an SSE subscription to both through the client's `Tracker` (H6), refetching on
  each newer tick and reopening the stream whenever it ends.
- **Fault sites** (`src/faults.rs`), on the store's commit points, all `buggify!` and so
  inert without `--buggify`, none during setup: a commit waits before it begins (short, or a
  few times per run past the attempt timeout) or holds its transaction open just before it
  commits (short, or once per run past the attempt timeout). Without them a run
  reaches two overlapping commits too rarely: a commit holds nothing long enough for another
  request, or a resubmission, to get ahead.
  Plus the client's `client-loses-response`: a first answer dropped on its way back.

When every client is done, the run waits for the views, reads the store's truth through the
service (not over the faulted network), and checks each invariant (`src/outcome.rs`). A
broken one is a `violation` verdict under its label (exit 1); otherwise a `pass` verdict
under `multiplayer-outcome` carries the counts, echoed as `MULTIPLAYER_RESULT`.

| Invariant | Holds when |
|---|---|
| `applied-once` | no patch id has two runs of events in the log |
| `acknowledged-visible` | every acknowledged patch is in its client's next snapshot and in the log at its receipt's revision |
| `nothing-unacknowledged-applied` | every patch in the log was acknowledged |
| `surfaced-not-applied` | no patch surfaced as a conflict is in the log |
| `only-conflicts-refused` | no patch got any rejection but stale, or a server error |
| `revisions-gap-free` | each domain's revision equals the number of its patches in the log (A17) |
| `applied-at-drafted-revision` | every acknowledged patch landed at the revision after the one it was drafted at, never past another commit (H5) |
| `replay-equals-state` | replaying the whole log from nothing gives the store's journey and deployment (J3) |
| `views-caught-up` | every view holds the store's revisions within 450 ms after writes stop: the coalescing interval plus one attempt's budget for the tick and the refetch (H6) |

Coverage oracles (`sometimes!`, plus the `reachable!` sites in `cairn_service`'s write path,
which the campaign also requires): a stale conflict surfaced, a resubmission answered from its receipt
(client and service), a coalesced tick, a deployment tick after the merge, a stale patch
completed with what intervened, a stale patch whose original committed meanwhile, a patch that lost at commit, and a resubmission that raced its
original to commit. Some of `cairn_service`'s sites are out of this testbed's reach and
listed in `sim.sh` (`out_of_reach`): the announce fallback when the deployment's revision
cannot be read back after an identity create riding in a journey patch (the testbed rides none
and fails no read); the derived-read memo (the views refetch the journey document, never a derived read); the
proposal and route-import paths (the testbed drafts no proposal and imports no route); and
the assistant's tool loop, turn limits, and write wrapper (the testbed never drives the
assistant).

Arguments: `--clients N`, `--actions N`. Exit
codes: 0 pass, 1 violation, 3 abort (runtime, store, or listener), 4 liveness (a client gave
up after 64 attempts, or the server would not stop), 64 bad arguments; never 2.

## Commands

Its own Cargo workspace: the check ladder formats and lints it (lint step) and runs its unit
tests (unit step), and only `mise run sim` simulates it; its
`Cargo.lock` started as the root's, so the crates under test resolve as `check` tests them.

```
cargo patina build . --output target/patina/cairn-multiplayer
cargo patina run target/patina/cairn-multiplayer --seed 1 [FAULT FLAGS]
mise run sim          # every leg of sim.sh, from the repository root
```

`sim.sh` (about 3 minutes): code (fmt, clippy, unit tests); build, and a clean audit; determinism (a seed
recorded twice is byte-identical, its replay reports the same verdict); the smoke campaign
(16 generations, `--buggify --sched-pct`, every oracle in reach fired, every generation
passing); the
faulted sweep (8 seeds of dropped, delayed, refused, and reset
connections, every run passing and resending, some patch answered from its receipt, no inert
knob).
The open patina limitation below is not checked: it exercises the pinned patina, not Cairn
(`decisions/2026-10-17-simulation-runs-no-known-gap-checks.md`).

When a smoke generation fails, read its observed verdict and replay the saved trace before
blaming determinism: Patina currently classes a finding found under PCT whose run hit a
change point as `NONDETERMINISM`, because a replay always reports a bug depth of 0 (a Patina
bug, reported).

## Open patina limitation

`campaign --faults` draws every plane with no way to leave
one out, so the network sweep is explicit (`decisions/2026-10-18-patina-campaign-faults-still-cannot-sweep-the-multiplayer-testbed.md`).

## Limits and choices

- One agent, four HTTP clients, eight patches each, three nodes: enough contention for every
  oracle within 16 generations, and short enough to stay under patina's trace event limit.
- The long faults are rationed per run (four waits, one stall): each long stall surfaces
  nearly every patch drafted during it, so more add nothing.
- The catch-up deadline covers the network as well as coalescing; under the sweep's faults
  the slowest view took about 280 ms of its 450.
