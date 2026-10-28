# The screenshot walk's checks and the breakpoint sweep run in the browser step on every push

**Question.** Work on checking screen structure by program added checks that only the screenshot walk makes (each fold opened, light and dark laid out the same, one Tab walk per screen for the focus ring, axe-core) and a sweep that visits every state at each width where the layout changes. The walk was outside the ladder, so a regression in any of these would show only when someone took pictures. Where do the walk and the sweep run?

**Measured.** Four workers, a developer machine, in-browser host: the screenshot walk 25 to 35 seconds for its 142 states (it was 18 before the new checks); the sweep 27 seconds for 17 widths (the thresholds the stylesheets name, one pixel either side, and 390, 560, 810, 960, 1440, 2560), about 10 seconds a width, 71 states each. The browser step of `check:fast` was about 33 seconds before.

**Call.** Both run in the browser step (`scripts/steps/browser`), so on every push of main and in `check:fast` for a change to web inputs. Together they add about a minute to a step that was about half of one, and they found real defects the first time they ran (a journey title's tap area under the touch minimum at 718px, the journeys table spilling out of its panel and past the window from 720 to 950px, a done chip under the contrast floor on the band, two popovers with no name, a context menu outside every landmark, tab buttons whose focus ring was cut at the top of the inspector, and graph lines with no focus ring). A check that runs only when someone asks is a check that lapses.

**Alternatives.** Nightly only: the sweep is cheap enough that a defect would not wait a day to be found, and the nightly run gives no failing commit to fix. A subset of widths on push and the full sweep nightly: two lists to keep, for a saving of about 20 seconds.

**What would change it.** The sweep growing past about a minute (more states, more widths: it scales with both), or the walk's checks flaking on a loaded runner. Then the sweep drops to nightly and the walk keeps the cheap checks on push.
