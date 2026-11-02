# The screenshot walk's checks and the breakpoint sweep run in the browser step on every push

Question: the structural checks only the screenshot walk makes (each collapsed section opened, both themes laid out alike, axe-core) and a sweep of every state at each width where the layout changes. Where do they run?

Call: both run in the browser step, so on every push of main and in `check:fast` for a change to web inputs. The sweep visits each width where a layout starts, the ones `web/app/src/shell/breakpoints.ts` names and the container-query switches they miss (`web/app/e2e/breakpoints.run.ts`). A check that runs only when someone asks is a check that lapses. Their cost is one visit per state per width, so it is held by a limit on the visits the walk and the sweep make together (`web/app/e2e/screenshots.ts`): a state or width past it fails the check instead of slowing every one.

Alternatives: nightly only (no failing commit to fix); a subset of widths on push and the full sweep nightly (two lists to keep for a few seconds saved).

What would change it: the walk's checks flaking on a loaded runner; then the sweep moves to nightly and the walk keeps the cheap checks. Reaching the visit limit is answered by cutting states, widths or what a visit costs; raising it is the owner's call.
