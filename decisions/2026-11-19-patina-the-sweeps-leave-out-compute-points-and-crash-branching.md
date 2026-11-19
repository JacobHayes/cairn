# [patina] the sweeps leave out compute points, crash-point branching and outside checks

Question: patina can rebuild the testbeds with compute points (thread switches inside computation), try every crash point of a seed (`--branch`), and judge runs from outside (`--check`, a faults-off reference run). Should the sweeps use them?

Call: no; on the testbeds each costs time and found nothing the plain sweeps do not. Measured with 8 jobs on a 24-CPU host:

- Compute points: the durability testbed runs on one thread, so they only cost (256 seeds in 8.7 s against 5.4 s, and a 44 s build of its own). The multiplayer testbed: 1,024 seeds in 58 s against 25 s plus a 70 s build, nothing found, and its rarest site met as often (281 runs against 275); at a short gap (`compute-gap=2..64`) 256 seeds took 310 s and 36 of them filled the log limit.
- Every crash point of two durability seeds is 3,256 runs in 44 s. The rarest crash oracles were met at about the random sweep's rate per CPU-second (a commit landed before the crash: 0.28 against 0.21; a crash between a commit's state rows and its events: 0.43 against 0.56), and nothing was found.
- Outside checks: the durability testbed judges recovery against its own durable ledger, which an outside checker would have to rebuild from the store and the engine's replay. A check that the store writes only under its data directory passed 256 seeds, but the testbed chooses that directory itself. The multiplayer outcome depends on the schedule, and a fault rightly changes which patches commit (stale conflicts), so a faults-off run is no reference; its invariants already say that a fault may fail a request but never change committed state.

Alternatives: compute points or branching in the nightly run only (a second build and minutes for a search that found nothing extra).

What would change it: a race in Cairn's code with no system call between its accesses, or a crash window the random sweep misses.
