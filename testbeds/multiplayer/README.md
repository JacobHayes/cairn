# Multiplayer testbed: many clients, one journey, under patina

The multiplayer testbed (PRACTICES, Simulation with patina). Cairn's real HTTP server, store,
and Rust client run unmodified under patina, with clients patching one journey through network
faults and fault sites in the store, and the multiplayer invariants checked at the end of
every run.

## The program

`src/main.rs` assembles a composition root as the binary's will (`src/world.rs`): the SQLite
store on a file in the run's virtual filesystem, the in-process notifier, the dev sign-in
provider (no token: every request is the dev user), and `cairn_api::router`, served by
axum on `127.0.0.1:7878`. Everything runs as tasks on a multi-thread tokio runtime with one
worker per CPU, as the binary serves (patina presents four), so requests run on several
threads and patina interleaves them.

Setup creates journey `j_shared` with three nodes, then one pair of identities drafted at the
deployment as the first request left it (its sign-in may create the caller's identity). Then:

- **Clients** (`--clients`, default 4; `src/client.rs`): each drafts `--actions` patches
  (default 8), a seeded pause apart, against the journey or deployment as it last fetched
  it: a note, a rename of one of the three nodes, and, for client 0 halfway through, an identity merge (moves the deployment). Each
  patch is sent once to the server (H5: it answers a stale patch stale, and the client surfaces it), over a transport
  retry that resends a request whose answer did not arrive within its budget (200 ms, doubling
  on each retry, as a client backs off on a slow link), unchanged, so a patch that landed is
  answered from its receipt. After each acknowledgement the client
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
service (not over the faulted network), and checks each invariant (`src/outcome.rs`). The
broken ones are reported together as one patina `violation` under the first one's label
(exit 1); the counts go out as `MULTIPLAYER_RESULT`.

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
| `views-caught-up` | every view holds the store's revisions within a minute after writes stop (H6): a liveness bound, since a faulted link takes what it takes; how soon a tick follows a commit is the notifier's deterministic tests' to check |

Coverage oracles (`sometimes!`, plus the `reachable!` sites in `cairn_service`'s write path,
which the sweeps also require): a stale conflict surfaced, a resubmission answered from its receipt
(client and service), a coalesced tick, a deployment tick after the merge, a stale patch
completed with what intervened, a patch that lost at commit, and a resubmission that raced its
original to commit. The sites out of this testbed's reach, and why, are in `patina.toml`.

Arguments: `--clients N`, `--actions N`. Exit
codes: 0 pass, 1 violation, 3 abort (runtime, store, or listener), 4 liveness (a client gave
up after 64 attempts, or the server would not stop), 64 bad arguments; never 2.

## Commands

Its own Cargo workspace: the check ladder formats and lints it (lint step), runs its unit tests
(unit step) and sweeps it (sim step: 256 seeds, CI's nightly run 2048); its `Cargo.lock`
started as the root's, so the crates under test resolve as `check` tests them. `patina.toml`
holds the sweep's settings (buggify and the network faults) and the sites out of reach.

```
cargo build --release
patina sweep --seeds 1..256                                      # the step's sweep, from here
patina run --seed 1 [--buggify] [--faults net] -- target/release/cairn-multiplayer
```

A finding's `rerun:` line runs it again from any directory.

## Limits and choices

- One agent, four HTTP clients, eight patches each, three nodes: enough contention for every
  oracle within the step's seeds.
- The long faults are rationed per run (four waits, one stall): each long stall surfaces
  nearly every patch drafted during it, so more add nothing.
- The network faults deliver pieces of at least 256 bytes (`patina.toml`): the preset's
  smallest, under its per-piece delays, carry a few bytes a second, and no request finishes.
