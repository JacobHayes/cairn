# A stage keeps its title, and the next milestone stays in its box

Two long-content cases now hold up. A zoomed-out stage whose title is one very long word shows a readable start of that word and an ellipsis instead of a huge state chip and a speck of title. The journey overview's next milestone is cut to two lines with an ellipsis (its full title on hover) and its date (with where it comes from on hover) reads right under it, as the other lists' titles do.

## A zoomed-out stage with one very long word for a title

Before: the chip takes the whole head.
![before](before-stage-light.png)

After: the title keeps the room at the reading size and the chip goes.
![after](after-stage-light.png)

Dark, before and after.
![before dark](before-stage-dark.png)
![after dark](after-stage-dark.png)

## The overview's next milestone, with the longest title a node may have

Before: a paragraph of capitals with the date trailing at its end.
![before](before-next-milestone-light.png)

After: two lines and an ellipsis, the date beneath.
![after](after-next-milestone-light.png)

## Known limits

- The stage's whole title is in the inspector; a one-word title wider than the head is only ever a prefix on the graph.
