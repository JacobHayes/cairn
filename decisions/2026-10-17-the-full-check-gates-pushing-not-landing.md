# The full check gates pushing, not landing

Question (from speeding up unattended units): every unit ran the full `mise run check` on its rebased result before landing, and units landing close together each paid for it in full. Most of its time is the integration-test binaries and the production web build, which `check:fast` skips. Should every landing pay for the full check?

Call: a unit lands once `mise run check:fast` passes on its rebased result. Before main is pushed, the full `mise run check` runs once on main, covering every unit landed since the last push; if it fails, nothing is pushed until a fix lands on main. CI still runs the full check on every push.

Alternatives: the full check before every landing (the old rule; one full check per unit, and units wait on it); no local full check, relying on CI (a failure reaches the public main and takes a CI run, about 20 minutes, to show).

What would change it: a failure of the push check that is hard to trace to one of the batched units, or main staying unpushable for long because landings keep arriving on a broken main.
