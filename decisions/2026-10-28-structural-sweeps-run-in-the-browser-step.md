# The screenshot walk's checks and the breakpoint sweep run in the browser step on every push

Question: the structural checks only the screenshot walk makes (each fold opened, both themes laid out alike, a Tab walk for the focus ring, axe-core) and a sweep of every state at each width where the layout changes. Where do they run?

Call: both run in the browser step, so on every push of main and in `check:fast` for a change to web inputs. The sweep's widths are chosen by rule from the thresholds the stylesheets name (`web/app/e2e/breakpoints.run.ts`). Together they add well under a minute. A check that runs only when someone asks is a check that lapses.

Alternatives: nightly only (no failing commit to fix); a subset of widths on push and the full sweep nightly (two lists to keep for a few seconds saved).

What would change it: the sweep growing past about a minute, or the walk's checks flaking on a loaded runner; then the sweep moves to nightly and the walk keeps the cheap checks.
