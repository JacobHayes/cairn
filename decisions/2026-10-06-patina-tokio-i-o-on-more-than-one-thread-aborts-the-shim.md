# [patina] tokio I/O on more than one thread aborts the shim

- Question: can a testbed run tokio's multi-thread runtime, or several current-thread runtimes on several threads, under the native shim?
- Call: no, at the pinned revision. A minimal tokio TCP ping-pong on a two-worker runtime aborts on 14 of 20 seeds with `patina native shim fatal: invalid_state: cannot wake scheduler task N in state Runnable` or `deadlock: no runnable tasks; parked tasks: ... (futex-wait), ... (epoll-wait)`; natively it passes 100 of 100. Task spawning and timers alone pass; the TCP I/O driver is what breaks. The spike's multi-thread and thread-per-client flavors fail 6 of 40 fault-free seeds each. Reproducer and outputs: `testbeds/spike/README.md`, gap 1; `mise run sim` fails when it stops reproducing. Testbeds use one current-thread runtime.
- Alternatives: patching patina (out of scope: patina is read-only to Cairn); running the multi-thread flavor anyway and discarding aborted seeds (hides the abort class and wastes the campaign).
- What would change it: a patina fix; then the multi-thread legs in `testbeds/spike/sim.sh` start failing and this entry and the README section are updated.
