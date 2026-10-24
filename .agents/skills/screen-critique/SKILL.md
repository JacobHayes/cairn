---
name: screen-critique
description: Critique Cairn screens from their screenshots and the code behind them, as a demanding product designer, against DESIGN.md and the change's intent. Use when a change alters what a screen shows or how it behaves, once its screenshots exist; mise run review runs it when given screenshots. Changes nothing.
---

# Screen critique

Judge the screens as a demanding product designer would, for someone opening Cairn for the first time, against DESIGN.md and what the change means to do. Read the screens' code as well as the pictures. When the screenshots come from `mise run screenshots`, read each one's `.checks.json` (the rules it passed, and what came near them: text cut by an ellipsis, the smallest hit areas, the font sizes in use) and `.aria.yml` (its roles and names, as a screen reader meets it; the manifest lists both) beside the picture: they are measured facts, so a finding about size, cut text or structure cites them. Change nothing.

Answer in three parts, in this order, most severe first within each:

1. **Structure**, judged by DESIGN.md's principles before anything else: what should not be there, what is in the wrong place, what reads differently from its siblings or the page the user came from, and what a newcomer would misread. Compare sibling views where you have them.
2. **Behaviour a user would trip on**, found in the code rather than the picture. Not code quality.
3. **Departures from DESIGN.md's tokens, type and spacing**, together in one closing bullet so they never crowd out the parts above, including where following DESIGN.md gives an awkward result.

Anything that looks broken is a finding, however small. For each: the screenshot, what is wrong, why it matters to a user, and what you would do. No praise.
