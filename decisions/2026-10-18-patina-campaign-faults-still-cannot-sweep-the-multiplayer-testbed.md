# [patina] campaign --faults still cannot sweep the multiplayer testbed

- Question: with the datagram-count fix in the pinned revision, can the multiplayer smoke campaign draw network faults with `--faults` (patina-bump)?
- Call: no. `d3162fd1` fixed the opportunity count (a TCP-only guest is no longer vacuous on duplicates) and ranks a guest failure above vacuity, but `--faults` still draws every plane, the filesystem with the network: on the multiplayer store that ended 12 of 16 generations in store errors (10 failed opens or first writes, 2 a store failed closed), 3 in network give-ups, 1 OK. The campaign stays `--buggify --sched-pct`, and leg 5's explicit network sweep stays. Two Patina problems found on the way are written up for Patina: a campaign classes every finding found under `--sched-pct` whose run hit a change point as `NONDETERMINISM` (a replay reports bug depth 0), which `sim.sh` fails on but names wrongly; and `--faults` has no way to leave a plane out.
- Alternatives: `--faults` anyway (the campaign would mostly measure store errors, not the network question); `--allow-unmet-sometimes` style waivers (hide the gap).
- What would change it: a campaign that can draw only the net plane.

Supersedes `2026-10-06-patina-campaign-faults-is-unusable-for-a-tcp-only-guest.md`.
