# The tour spotlights the action, then shows what changed

Question: the tour teaches by having a person act on a practice journey. How does it point at the action without hiding what the action changes?

Call: a step that asks for an action has two states. While the action waits, the screen dims around the action area, the guidance sits in a callout beside it, and nothing outside can be clicked. When the derived state shows the action done, the dimming lifts or widens to take in the change, and the callout moves beside the change and says what Cairn worked out. The person moves on with Next, never a timer, and Escape never skips the tour. The callout is placed by the shared `place()` and never covers its target; target rectangles are read when something could move them, never on a clock.

Alternatives: a band above the screen (the instruction sits far from the action and takes a fifth of the workspace); a spotlight that stays dimmed after the action (it hides the consequence the step exists to show); a dedicated practice screen (a reduced app).

What would change it: new people losing the thread between the spotlight and the reveal, or a second guided flow wanting the same two states.
