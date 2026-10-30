# [patina] campaign --faults cannot sweep the multiplayer testbed

Question: can the multiplayer smoke campaign draw network faults with `--faults`?

Call: no. `--faults` draws every fault plane, the filesystem with the network, and on the multiplayer store most generations end in store errors rather than answering the network question. The campaign runs `--buggify --sched-pct`, and an explicit network sweep leg draws the network faults. Two Patina gaps stand behind this: `--faults` cannot leave a plane out, and a campaign under `--sched-pct` classes findings whose run hit a change point as `NONDETERMINISM`.

Alternatives: `--faults` anyway (it would mostly measure store errors); waivers for unmet `sometimes!` (hide the gap).

What would change it: a campaign that can draw only the network plane.
