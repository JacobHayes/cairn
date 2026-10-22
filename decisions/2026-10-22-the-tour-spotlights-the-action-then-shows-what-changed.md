# The tour spotlights the action, then shows what changed

Question: the tour of Cairn teaches by having a person act on a practice journey. How does it point at the action without hiding what the action changes? This comes from the comparison of an overlay with the band after the band shipped.

Call: steps that ask for an action have two states. While the action waits, the screen dims and the complete action area is cut out, with the guidance in a callout beside it; nothing outside can be clicked and the app's keys are quiet. When the derived state shows the action done, the dimming lifts, or widens to take in the change when it grows out of the action area as one box, and the callout moves beside the change and says what Cairn worked out. The person moves on with Next, never a timer. The callout is placed by the shared tooltip placement, so the tour adds no second floating mechanism. Escape never skips the tour. This changes the call in `2026-10-22-the-tour-is-the-one-guided-first-run.md`, which chose a band above the screen and no dimming.

Calls made while building it:

- The overlay reads the target rectangles once per animation frame and re-places only when one changed, rather than subscribing to each source (canvas pans, scrolls, sheet drags, disclosures and mounts). One loop covers every way a target can move, and a state is drawn once its rectangles have held for a few frames.
- A target that is missing is told as loading, closed (the sheet is lowered) or missing (after two seconds), with a button that shows the control; the finer diagnoses of the plan collapse into these.
- Each step also names what its sentence is about, and the callout is placed so it never covers those: it tries each side and alignment beside its anchor, then docks across the foot of the room above the sheet, then across its head. A phone, a window too short for it to sit beside a card, and an anchor inside the sheet dock at once; in a window under 480px high the guidance moves to the side of a card instead of above or below it, so the card keeps room to be read.
- The canvas, which owns its camera, frames the step's cards in the room the callout leaves, down to a near zoom where a card still shows what it is. The overlay judges docking only after the canvas has framed.
- Under the sheet (below 1100px) the hole takes in the sheet's handle with the target, so the person can still resize it; Escape on the paused card resumes. The callout is paper, the app's open layer: it blocks the app's keys and keeps focus.
- The unit tests of geometry, focus order and step facts run in Vitest's node environment (there is no DOM there); what the screen does is covered by the browser tests.

Alternatives: the band above the screen (it kept the screen whole but put the instruction far from the action and took a fifth of the workspace); a spotlight that stays dimmed after the action (it hides the consequence the step exists to show); a dedicated practice screen (a reduced app shown to the person).

What would change it: new people losing the thread between the spotlight and the reveal, or a second guided flow wanting the same two states as a shared component.
