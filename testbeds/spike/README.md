# Simulation spike: tokio, axum, and an HTTP client under the patina shim

Brief 1.3. The question: can the patina native shim run Cairn's HTTP stack (axum on tokio)
and an HTTP client, deterministically and with network faults reaching them? The answer
decides whether 6.1's multiplayer testbed drives the real HTTP server or calls the service
in-process. This directory is throwaway: brief 6.2 folds it in or deletes it.

**Answer: yes, on one current-thread tokio runtime with a ticker task; no, on more than one
thread doing tokio I/O.** 6.1 drives the real HTTP server (DECISIONS.md, 2026-10-06, brief
1.3). Everything below is real output from patina at the pinned revision `6a2977e7` on
Linux x86_64; three patina gaps are written up at the end.

## The program

`src/main.rs` runs an axum 0.8 server and three hyper 1 clients on tokio 1.53.1, over the
loopback network patina simulates. Each client appends 8 request ids to the server's ledger
with `POST /append`, one fresh HTTP/1 connection per attempt, retrying a failed or timed-out
attempt (40 ms timeout, 5 ms backoff, 64 attempts); the server answers a retried id with its
first revision. A final `GET /log` reads the ledger, and the run checks 6.1's invariants:
every acknowledged request is in the log at its acknowledged revision (`acknowledged-visible`),
none is applied twice (`applied-once`), the log holds nothing unacknowledged
(`no-phantom-entries`) and parses (`log-well-formed`). The outcome goes through patina's
verdict channel (`pass` under `spike-outcome`, or a `violation` per broken invariant), echoed
as a `SPIKE_RESULT` line: attempts, retries, deduplicated requests, virtual time elapsed, a
digest of the acknowledged ids (the same in every passing run), and a digest of the log order
(which depends on the schedule).

Arguments: `--flavor current-thread|multi-thread|thread-per-client` (default
current-thread; multi-thread has two workers; thread-per-client runs each client on its own
`std::thread` with its own current-thread runtime), `--tick-ms N` (a task that wakes the
runtime every N ms; see gap 2), `--bug no-dedupe` (the planted server bug), `--clients`,
`--requests`. Fault sites: `client-loses-response` (`buggify!`: the client drops the server's
answer on a first attempt). Coverage oracles: `client-retried` and `server-deduplicated-retry`
(`reachable!`).

## Build flags and commands

Its own Cargo workspace (an empty `[workspace]` table), so `cargo patina build` resolves only
this tree and `mise run check` never builds it. Debug profile (patina's bug-finding profile),
no `--yield-points`, no extra cfgs; the SDK is `patina-dst` at the pinned revision.

```
cargo patina build . --output target/patina/cairn-spike
cargo patina audit target/patina/cairn-spike
cargo patina run target/patina/cairn-spike --seed 1 [FAULT FLAGS] -- --tick-ms 1
mise run sim          # every leg below, from the repository root (testbeds/spike/sim.sh)
```

`mise run sim` takes about 15 s here: code checks and unit tests, build, audit, determinism,
the smoke campaign, the faulted sweep, and one leg per gap that fails if the gap stops
reproducing (so a patina bump that fixes one gets this file and DECISIONS.md updated).

## Results

### Audit: clean, no allowance

```
$ cargo patina audit target/patina/cairn-spike
(84 lines: the libc and pthread symbols the shim interposes, then two notes)
note: 15 linked symbol(s) are deny-trap armed under patina (a call aborts deterministically): execvp (process), fork (process), ...
host-identity reads (cpuid, 12 sites): unmanaged — patina neither traps nor models the host CPU's identity ...
exit status 0
```

No `--allow` and no `--allow-unsupported-symbols`: tokio, mio, hyper, and axum reach only
effects the shim models. The process-spawn symbols are linked by std and never called; the
`cpuid` reads (std's feature detection) cost cross-host reproducibility only.

### Fault-free runs: every flavor, byte-identical repeats

Each seed recorded twice (`--record`); trace hash and log-order digest:

| Flavor | Seed | Exit | Trace (repeat a) | Trace (repeat b) | Log order |
|---|---|---|---|---|---|
| current-thread | 1 | 0, 0 | `31359a886c53` | `31359a886c53` | `e67ad9c5f85ce97d` |
| current-thread | 2 | 0, 0 | `e6ac874074c5` | `e6ac874074c5` | `e67ad9c5f85ce97d` |
| current-thread | 3 | 0, 0 | `56ee7bda16c5` | `56ee7bda16c5` | `e67ad9c5f85ce97d` |
| multi-thread | 1 | 0, 0 | `5d010ff0194c` | `5d010ff0194c` | `8adfdd9d05438aeb` |
| multi-thread | 2 | 0, 0 | `ff5c95de96f9` | `ff5c95de96f9` | `512567873ff13725` |
| multi-thread | 3 | 0, 0 | `16f7eeb666a1` | `16f7eeb666a1` | `ffd39aa1fa59827b` |

`cargo patina replay` of a recording reproduces its verdict (multi-thread seed 3 replays to
`order=ffd39aa1fa59827b`). The current-thread log order is the same at every seed (one
thread, nothing for the seed to reorder); the multi-thread order varies by seed, as it
should. A run takes about 0.3 s of wall time.

Over seeds 1 to 40, fault-free with `--tick-ms 1`: current-thread passes all 40;
multi-thread and thread-per-client each abort on 6 with a shim fatal (gap 1).

### Faults reach the program and change the outcome

`briefs/proof/1.3/README.md` (generated by `briefs/proof/1.3/prove.sh`) shows one seed
fault-free and under each fault: resets and refused connects force retries, resets after the
server applied a request make it deduplicate the retry, delays stretch virtual time, and the
planted `no-dedupe` bug passes fault-free but fails `applied-once` once resets force
retries. Summary at seed 1, current-thread, `--tick-ms 1`:

| Run | Exit | Retries | Deduplicated | Elapsed (us) |
|---|---|---|---|---|
| fault-free | 0 | 0 | 0 | 0 |
| `--net-reset-permille 100` | 0 | 37 | 11 | 95000 |
| `--net-connect-refuse-permille 200` | 0 | 5 | 0 | 20000 |
| `--net-latency-nanos 2000000 --net-jitter-nanos 0..2000000 --net-drop-permille 100` | 0 | 0 | 0 | 69000 |
| `--buggify=1000 --buggify-activation-permille 1000` | 0 | 25 | 24 | 45000 |
| `--bug no-dedupe`, fault-free | 0 | 0 | 0 | 0 |
| `--bug no-dedupe`, `--net-reset-permille 100` | 1 | 37 | 0 | 95000 |

The faulted sweep (`mise run sim` leg 5) runs drop 100, latency 1 ms, jitter up to 2 ms,
refuse 100, and reset 100 per mille together:

```
$ cargo patina explore run target/patina/cairn-spike --seeds 8 --net-drop-permille 100 --net-latency-nanos 1000000 --net-jitter-nanos 0..2000000 --net-connect-refuse-permille 100 --net-reset-permille 100 -- --tick-ms 1
SPIKE_RESULT flavor=CurrentThread requests=24 acknowledged=24 log=24 attempts=69 retries=44 deduplicated=10 elapsed_us=242000 ids=e072ebcbd5b613f5 ...
SPIKE_RESULT flavor=CurrentThread requests=24 acknowledged=24 log=24 attempts=81 retries=56 deduplicated=19 elapsed_us=248000 ids=e072ebcbd5b613f5 ...
(6 more, every one passing with retries between 20 and 56)
PATINA_EXPLORE_COMPLETE start=0 seeds=8
```

The same sweep over 200 seeds: all pass, 13 to 71 retries each, 1,978 deduplicated retries in
all. Under the harshest single settings tried (reset and refuse 200 per mille with a 64-byte
TCP buffer), seed 2 exhausts a client's 64 attempts (`SPIKE_FAILURE ... gave up on 2-1: io:
Connection refused`): the retry budget's limit, not a patina gap.

### Smoke campaign

```
$ cargo patina campaign target/patina/cairn-spike --gens 16 --buggify --sched-pct --out-dir target/patina/campaign -- --tick-ms 1
== campaign summary ==
generations=16 failures=0 novel_signatures=0
  class OK                 16
-- coverage (sometimes!/reachable!) --
oracle_sites=2 satisfied=2 unmet=0
PATINA_CAMPAIGN_COVERAGE oracle_sites=2 satisfied=2 unmet=0 gate=pass
```

The campaign leaves out `--faults` (gap 3); the faulted sweep above covers the network
faults instead. The same campaign with `--flavor multi-thread` fails 7 of 16 generations as
`FAIL_CLOSED_ABORT` (`refusal class=shim_fatal`, gap 1).

## What works and what does not

| Shape | Fault-free | Network faults | Notes |
|---|---|---|---|
| current-thread runtime, axum server and hyper clients as tasks, `--tick-ms 1` | works | works | the configuration 6.1 uses |
| current-thread, no ticker | works | delayed segments stall to the next timer (gap 2) | resets and refused connects work |
| multi-thread runtime (2 workers) | aborts on some seeds (gap 1) | aborts | |
| a current-thread runtime per client thread | aborts on some seeds (gap 1) | aborts | |
| tokio server, blocking `std::net` clients on threads | works (reproducer only, 20 of 20 seeds, also with latency) | | not built into the spike |

Not tried: TLS (patina models none), DNS (IP literals only; patina's `--dns-entry` exists),
SSE or other long-lived streaming responses, reqwest or other client crates, HTTP/2.

## Patina gap report

Three gaps, each reproduced at the pinned revision and confirmed absent natively. Each has a
`DECISIONS.md` entry tagged patina and a leg in `sim.sh` that fails once it stops
reproducing. The reproducer is one file, a tokio TCP ping-pong (dependency: `tokio =1.53.1`
with `macros`, `net`, `rt`, `rt-multi-thread`, `io-util`, `time`):

```rust
// tokio TCP ping-pong under the patina native shim.
// usage: mre RUNTIME [TIMER]
//   RUNTIME: current | multi (two workers)
//   TIMER:   none (default) | armed: a 1 s tokio timeout around the exchange
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let runtime = match args.get(1).map(String::as_str) {
        Some("multi") => tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build(),
        _ => tokio::runtime::Builder::new_current_thread().enable_all().build(),
    }
    .unwrap();
    let armed = args.get(2).map(String::as_str) == Some("armed");
    runtime.block_on(async move {
        let listener = TcpListener::bind("127.0.0.1:7000").await.unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4];
            stream.read_exact(&mut buf).await.unwrap();
            stream.write_all(b"pong").await.unwrap();
        });
        let exchange = async {
            let mut stream = TcpStream::connect("127.0.0.1:7000").await.unwrap();
            stream.write_all(b"ping").await.unwrap();
            let mut buf = [0u8; 4];
            stream.read_exact(&mut buf).await.unwrap();
        };
        let start = Instant::now();
        if armed {
            tokio::time::timeout(Duration::from_secs(1), exchange).await.unwrap();
        } else {
            exchange.await;
        }
        println!("MRE_OK exchange took {:?} of virtual time", start.elapsed());
        server.await.unwrap();
    });
}
```

Natively, every mode passes 100 of 100 runs and the exchange takes about 100 us.

### Gap 1: tokio I/O on more than one thread aborts the shim

```
$ cargo patina run mre --seed 1 -- multi
patina native shim fatal: deadlock: no runnable tasks; parked tasks: 1 (futex-wait), 2 (futex-wait), 3 (epoll-wait)
PATINA_INFRA native_run signal=6
exit 134
$ cargo patina run mre --seed 2 -- multi
patina native shim fatal: invalid_state: cannot wake scheduler task 3 in state Runnable
PATINA_INFRA native_run signal=6
exit 134
$ cargo patina run mre --seed 5 -- multi
MRE_OK exchange took 0ns of virtual time
exit 0
```

Seeds 1 to 20: 6 pass, 6 `deadlock`, 8 `invalid_state`. A multi-thread runtime that only
spawns tasks or sleeps passes every seed; the TCP I/O driver is what breaks it, and the
spike's thread-per-client flavor (separate current-thread runtimes on separate threads)
fails the same way (`invalid_state: cannot wake scheduler task 1 in state Runnable`, 6 of 40
seeds). Blocking `std::net` threads beside a current-thread tokio server pass. Likely shape:
the shim's scheduler and its epoll model disagree when a thread blocked in `epoll_wait` and
a thread parked on a futex are both woken for one readiness event. Patina's own tokio
testbeds (`pubsub`, `native-boundary/tokio`) run current-thread only, so nothing covered it.

### Gap 2: a delayed TCP segment does not wake `epoll_wait`

```
$ cargo patina run mre --seed 1 -- current
MRE_OK exchange took 0ns of virtual time
$ cargo patina run mre --seed 1 --net-latency-nanos 1 -- current armed
MRE_OK exchange took 1s of virtual time
$ cargo patina run mre --seed 1 --net-latency-nanos 2000000 -- current armed
MRE_OK exchange took 1s of virtual time
$ cargo patina run mre --seed 1 --net-latency-nanos 1 -- current
patina native shim fatal: a shim lock was re-entered by the thread that holds it (a signal handler ran over shim code); failing closed
PATINA_INFRA native_run signal=6
exit 134
```

With any delay on the TCP path (`--net-latency-nanos`, `--net-jitter-nanos`, or a drop's
retransmit backoff), a segment whose delivery time passes while the reader is in
`epoll_wait` is not delivered then: the reactor sleeps to its own timeout (1 ns of latency
costs the full 1 s timer), and with no timer armed the shim aborts. Blocking `std::net`
reads under the same latency are delivered on time. In the spike without a ticker, every
attempt times out at 40 ms and the run gives up (`sim.sh` gap leg `delayed-delivery`: 192
timed-out attempts, then `SPIKE_FAILURE ... gave up on 0-0: timed out`). The workaround is
`--tick-ms 1`, a task that wakes the runtime every millisecond, which bounds the lag to the
tick; with it every delayed run above passes and no attempt times out. Patina's `pubsub`
testbed passes its latency leg because its 40 ms heartbeat timers do the same job.

### Gap 3: `campaign --faults` is unusable for a TCP-only guest

```
$ cargo patina campaign target/patina/cairn-spike --gens 4 --faults --buggify --out-dir target/patina/campaign-faults -- --tick-ms 1
== campaign summary ==
generations=4 failures=4 novel_signatures=1
  class VACUOUS_NET_FAULT  4
-- failure signatures --
  [VACUOUS_NET_FAULT] count=4 first_gen=0 seed=5133223892554006150
      signature: VACUOUS_NET_FAULT|vacuous plane=net|
```

The `--faults` bands draw `--net-duplicate-permille` in every generation, but duplication
acts on datagrams only, so a guest that uses only TCP reports `duplicates_applied=0` and the
generation is classed `VACUOUS_NET_FAULT` (replaying generation 0 shows the spike itself
passed: `SPIKE_RESULT ... retries=34 deduplicated=19`). There is no way to drop one band
from a campaign spec, and `classify` rules cannot override a vacuity class. Worse, the
vacuity class masks the guest's own outcome: the same campaign without `--tick-ms 1` gives
up in every generation (exit 4) and is still classed `VACUOUS_NET_FAULT` 10 of 10. A smaller
related effect: the net vacuity diagnostic is a hard zero-count test, so a live knob that
draws no effect by chance is flagged inert (the faulted sweep at seed 158 makes 55 connects
at 100 per mille refusal and draws 0 refusals, probability about 0.3%, and reports
`vacuous=1`). Until this changes, the spike's campaign runs without `--faults` and the
network faults run as an explicit-knob sweep (`cargo patina explore run`).
