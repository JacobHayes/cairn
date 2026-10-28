# The Stages list runs in due-date order

Question: in what order does the journey card's Stages list show the stages, from the work that
adds a Stages list to the journey card? The plan asked for the canvas's layout order.

Call: by due date, earliest first, stages with no due date after, equal dates in the document's
order. The card is drawn on every view, including ones with no canvas, so the canvas layout is not
there to read; the layout orders stages by what they wait on, and a stage that waits on another is
due no earlier than it, so the two agree wherever stages are ordered by waiting.

Alternatives: reading the order from the canvas layout (a store the canvas writes and the card
reads, empty on views without a canvas); ordering by what each stage waits on in the card (a second
copy of the layout's layering).

What would change it: stages whose dates disagree with their waiting order reading wrong in the
list, or a view model that exposes the stages' laid-out order to every view.
