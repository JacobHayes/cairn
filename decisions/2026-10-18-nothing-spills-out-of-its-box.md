# Every browser test checks that nothing spills out of its box

## Question

Raised by owner reports of text and controls sticking out of their boxes on the deployed app (a
stage card's expand button past its border, titles over the foot, foot dates under the border).
Each was fixed one at a time and the UI critique missed all of them. How is this prevented
systematically?

## Call

A containment invariant in the browser harness (`web/app/e2e/containment.ts`), run by the `test`
every spec imports (`e2e/test.ts`, an auto fixture; eslint forbids importing Playwright's
`test` in `e2e/`) on every page a test opened, and by `capture` before a proof screenshot. It
checks geometry, not words: each drawn element's box lies inside its nearest bounded ancestor
(card, panel, callout, chip, badge, button, row, inspector section, toolbar, header, and an
overhanging element for what it holds) within 0.5 px, and an element holding text whose content is
wider than it, with no ellipsis, clamp or scroll, is a failure. What a clipping ancestor hides is
compared after the clip (its own cut is the second rule's). A scroll container or a sticky or
fixed ancestor excuses the axis it moves. What is not drawn (a closed disclosure, a visually
hidden element) is skipped. The exceptions are DESIGN's overhangs, marked by one attribute,
`data-overhang`: hanging and rank tags, a control's count badge (added to DESIGN's Hanging tag
here, since it was the one corner badge in use), connection handles, popovers, menus and the
keys tooltip. The old `expectRowsInsideCards` is replaced by it. A stress journey
(`e2e/stress.ts`) of worst-case content is walked through every screen, width and theme.

## Alternatives

- An opt-in helper called per spec: the reports show it would be forgotten.
- A visual-diff or screenshot-only check: slow, and says nothing about which element escaped.
- A selector allow-list in the test: it would rot; the marker lives with the element.

## What would change it

Frequent false positives from a new component pattern: add its box to `BOUNDED` or mark its
overhang, in the change that adds it.
