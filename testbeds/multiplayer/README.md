# Multiplayer testbed: many clients, one journey, under patina

Brief 6.1, on the simulation track (PRACTICES, Simulation with patina): exploratory, run by
`mise run sim`, never by `mise run check`. Cairn's real HTTP server, store, and Rust client
under the patina native shim, with clients patching one journey through network faults and
fault sites in the store, and the multiplayer invariants checked at the end of every run.
Proof: `briefs/proof/6.1/README.md`.

## The program

`src/main.rs` assembles a composition root as the binary's will (`src/world.rs`): the Turso
store on a file in the run's virtual filesystem, the in-process notifier, the dev sign-in
provider (no token: every request is the dev user), and 4.2's `cairn_api::router`, served by
axum on `127.0.0.1:7878`. Everything runs as tasks on a multi-thread tokio runtime with four workers, so requests
run on several threads and PCT reorders them
(`decisions/2026-10-18-the-multiplayer-testbed-runs-a-multi-thread-runtime.md`).

Setup creates journey `j_shared` with three nodes and one pair of entities. Then:

- **Clients** (`--clients`, default 4; `src/client.rs`): each drafts `--actions` patches
  (default 8), a seeded pause apart, against the journey or deployment as it last fetched
  it: a note (touches only its own annotation, so a stale one is always safe to resubmit), a
  rename of one of the three nodes (two renames of one node overlap, so some conflicts are
  surfaced), and, for client 0 halfway through, an entity merge (moves the deployment). Each
  patch goes through 4.2's `client::retry::submit` (H5: a stale patch is rebased and
  resubmitted when what intervened cannot overlap it, surfaced otherwise) over a transport
  retry that resends a request whose answer did not arrive within 200 ms, unchanged, so a
  patch that landed is answered from its receipt. After each acknowledgement the client
  fetches the domain again: the next snapshot must hold the patch.
- **The agent** (one more client, numbered after the others): the same drafts and the same
  retry logic, calling the service in process as an agent host (MCP, the assistant) does.
  Each attempt runs on its own task, so an attempt the agent stops waiting for keeps going.
  An HTTP client cannot do this: the server drops a request whose connection closes, so a
  resubmission over HTTP never meets its original in the server (DECISIONS.md, 6.1).
- **Views** (one per HTTP client; `src/view.rs`): fetch the journey and the deployment, then
  follow an SSE subscription to both through 4.2's `client::Tracker` (H6), refetching on
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
| `retried-only-when-safe` | a patch that landed past the revision it was drafted at landed over no commit touching what it touches (H5) |
| `replay-equals-state` | replaying the whole log from nothing gives the store's journey and deployment (J3) |
| `views-caught-up` | every view holds the store's revisions within 450 ms after writes stop: the coalescing interval plus one attempt's budget for the tick and the refetch (H6) |

Coverage oracles (`sometimes!`, plus the `reachable!` sites in `cairn_api::client::retry` and
`cairn_service`'s write path, which the campaign also requires): a stale patch retried on its
own and landed, an overlapping conflict surfaced, a resubmission answered from its receipt
(client and service), a coalesced tick, a deployment tick after the merge, a stale patch
completed with what intervened, a patch that lost at commit, and a resubmission that raced its
original to commit. Some of `cairn_service`'s sites are out of this testbed's reach and
listed in `sim.sh` (`out_of_reach`): the announce fallback when the deployment's revision
cannot be read back after an entity create riding in a journey patch (the testbed rides none
and fails no read); the receipt recheck after the engine answers stale (it needs the original
to commit between a resubmission's receipt lookup and its load, and no site delays a load);
the derived-read memo (the views refetch the journey document, never a derived read); the
proposal and route-import paths (the testbed drafts no proposal and imports no route); and
the assistant's tool loop, turn limits, and write wrapper (the testbed never drives the
assistant).

Arguments: `--clients N`, `--actions N`. Exit
codes: 0 pass, 1 violation, 3 abort (runtime, store, or listener), 4 liveness (a client gave
up after 64 attempts, or the server would not stop), 64 bad arguments; never 2.

## Commands

Its own Cargo workspace: the check ladder formats and lints it (lint step) but never runs it,
and only `mise run sim` does; its
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
knob). The patina gaps below are not checked: they exercise the pinned patina, not Cairn
(`decisions/2026-10-17-simulation-runs-no-known-gap-checks.md`).

## The product finding (fixed)

H5, found by the smoke campaign in about one generation in eight and recorded in
DECISIONS.md (brief 6.1): a patch resubmitted while its original was still committing was
answered stale, naming the revision the original was producing with nothing intervening. The
retry logic rebased it onto that revision; the engine answered that it named a revision that
did not exist yet; the client rebased back, and so on until its 32-resubmission bound ran
out in no time at all, and the caller was told the patch conflicted. Then the original
landed. The same stale answer, given to any other patch beside a commit in flight, named
that revision with nothing intervening even when the commit in flight touched what the patch
did. Fixed (DECISIONS.md, the same entry): a Turso commit first takes its turn at every
revision row it writes and at its patch id, so one beside a commit in flight on them waits
for it and then meets its receipt or what it really touched; and the client never rebases a
patch backward. `crates/tests/tests/integration/service/in_flight.rs` holds both cases over both stores, and
the campaign tolerates no finding.

When a smoke generation fails, read its observed verdict and replay the saved trace before
blaming determinism: Patina currently classes a finding found under PCT whose run hit a
change point as `NONDETERMINISM`, because a replay always reports a bug depth of 0 (a Patina
bug, reported).

## Patina gap history

The gaps this testbed found in Patina, each with a `DECISIONS.md` entry tagged patina. Gaps
1 to 3 are the spike's. Gaps 4, 5 and 6 are fixed at the pinned revision, and no leg checks
them:

- Gap 4, Turso links symbols the shim refuses to run beside (`dlopen`, `dlclose` and
  simsimd's AVX-512 FP16 kernels): the audit decodes those instructions and traps `dlopen`
  (`6bca33af`), so the audit is clean and no run names an allowance.
- Gap 5, a site label holding a space breaks a campaign: labels are escaped in the report
  (the label rule in `scripts/steps/lint` stays for the reasons in
  `decisions/2026-10-18-patina-main-retires-four-testbed-workarounds.md`).
- Gap 6, an `always!` violation leaves no trace to replay: the trace is finalized and
  replays (`2d1c8d3f`). The invariants stay `violation` verdicts for another reason
  (`decisions/2026-10-18-testbed-invariants-stay-verdicts.md`).

Also fixed, and removed from the testbed: a delayed TCP segment waking no reactor (the 1 ms
ticker; `0115ebf0`) and tokio TCP I/O on a multi-thread runtime aborting the shim (the
single-thread runtime; `0115ebf0`).

Still open and worked around: `campaign --faults` draws every plane with no way to leave
one out, so the network sweep is explicit (`decisions/2026-10-18-patina-campaign-faults-still-cannot-sweep-the-multiplayer-testbed.md`).

## Limits and choices

- One agent, four HTTP clients, eight patches each, three nodes: enough contention for every
  oracle within 16 generations, and short enough to stay under patina's trace event limit,
  which one early version (32 resubmissions per patch during every long stall) exceeded.
- The long faults are rationed per run (four waits, one stall): each long stall surfaces
  nearly every patch drafted during it (the finding above), so more add nothing.
- The catch-up deadline covers the network as well as coalescing; under the sweep's faults
  the slowest view took about 280 ms of its 450.
