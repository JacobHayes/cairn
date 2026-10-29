# The tour spotlights the action, then shows what changed

Question: the tour of Cairn teaches by having a person act on a practice journey. How does it point at the action without hiding what the action changes? This comes from the comparison of an overlay with the band after the band shipped.

Call: steps that ask for an action have two states. While the action waits, the screen dims and the complete action area is cut out, with the guidance in a callout beside it; nothing outside can be clicked and the app's keys are quiet. When the derived state shows the action done, the dimming lifts, or widens to take in the change when it grows out of the action area as one box, and the callout moves beside the change and says what Cairn worked out. The person moves on with Next, never a timer. The callout is placed by the shared tooltip placement, so the tour adds no second floating mechanism. Escape never skips the tour.

Calls made while building it:

- The overlay reads the target rectangles when something that could move them happens (a resize of the window or the callout, a scroll, a change of the page's markup), coalesced to one read per animation frame, never on a clock. The callout is placed once per read, beside its target or docked across the foot (or head) of the room above the sheet.
- A target that is not on the screen shows the callout docked after a short wait; where the sheet is lowered over a step that needs its pane, a button raises it.
- The callout is placed so it never covers its hole or its anchor: beside its anchor on the preferred side or the opposite, else docked across the foot of the room above the sheet, else across its head. A phone docks at once; in a window under 480px high the guidance moves to the side of a card instead of above or below it.
- The canvas, which owns its camera, frames the step's cards in the room the callout leaves, down to a near zoom where a card still shows what it is.
- The callout is paper, the app's open layer: it blocks the app's keys, and arriving at a step puts the focus on its title. Escape on the paused card resumes.
- The unit tests of step facts run in Vitest's node environment (there is no DOM there); what the screen does is covered by the browser tests.

Alternatives: the band above the screen (it kept the screen whole but put the instruction far from the action and took a fifth of the workspace); a spotlight that stays dimmed after the action (it hides the consequence the step exists to show); a dedicated practice screen (a reduced app shown to the person).

What would change it: new people losing the thread between the spotlight and the reveal, or a second guided flow wanting the same two states as a shared component.
