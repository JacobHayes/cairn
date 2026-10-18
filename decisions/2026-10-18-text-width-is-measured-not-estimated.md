# A card's title width is measured in the browser, not estimated

## Question

Raised by the work that keeps text and controls inside their boxes. `canvas/cards.ts` sized a
card's title lines from a character-class estimate (capitals and digits 0.78 em, other letters
0.58 em, a space 0.3 em). The browser wraps the real text in the real face, so drawing and
layout could disagree: a title the estimate counted as two lines that wraps to one left a blank
line in its card; one counted as one that wraps to two was clipped to one line with an ellipsis
for nothing. Should layout keep estimating, or measure?

## Call

Measure. `canvas/measure.ts` measures a word's width in ems with a canvas on the loaded title
face (the body font at weight 600, read from the `--font-body` token), and `boot.ts` loads that
face and installs the measure before any card is laid out. `cards.ts` reads widths only through
it (`emWidthOf`), so the line count layout reserves is the count the browser wraps to; the
character-class model is gone from production code. Vitest runs in Node, which has no fonts, so
`measure.test-support.ts` installs the old model as the test measure (a fixed model the unit
tests of the wrapping rule need); the browser tests measure the real face, and
`expectTitleLinesAsLaidOut` (e2e/canvas.ts) fails when a card's drawn lines differ from the
reserved ones. The stress journey carries one title the old estimate over-counted and one it
under-counted, so the check fails on the old model and passes on the measure.

Two estimates stay, by choice. An open container's header is sized before ELK gives the
container its width, so its title lines are counted for a card's width and its title may take
fewer (a blank line, never an overflow). And the zoomed-out stage chip's width
(`CHIP_LETTER_PX`) only decides whether an optional chip is shown; the containment invariant
checks the result.

## Alternatives

- Keep the estimate and widen its margins: always errs one way, leaving blank lines.
- Measure with a hidden DOM element per title: forces a layout per card and cannot run before
  the DOM is up; a canvas measure is synchronous and cached per word.
- Make the title row grow to what the browser wraps (no reserved lines): the layout (ELK) needs
  every height before it runs, so a drawn height that differs would overlap neighbours.

## What would change it

A face with several unicode-range subsets whose glyphs load after boot (a title in a script the
latin subset lacks) is measured in the fallback until the face arrives; if that drifts in
practice, re-measure and re-layout on `document.fonts` `loadingdone`.
