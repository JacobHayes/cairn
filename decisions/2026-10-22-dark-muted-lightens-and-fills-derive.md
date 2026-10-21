# Dark muted text is a step lighter, and diff fills derive from the state tokens

Question: the design tokens kept the add, change and remove fills as hand-written rgba values (the light theme's hues in both themes), and nothing measured them. Measured, muted text in dark mode on a fill was 3.85 to 4.56:1 on paper and the band, under the 4.5:1 floor the design states. How are the fills owned and the floor met? (Work: one owner for the design tokens.)

Call:
- Each fill is the state token (success, warning, error) at 8% in light and 10% in dark (a stronger fill drops the status colours set as text on a marked card, a DONE chip on a changed card, under 4.5:1), mixed in CSS, so a change to a state colour reaches its fill.
- Dark `--color-muted` moves from `--n-6` to `--n-5`, the only way to keep both a visible fill and 4.5:1 for muted text on the band under it (at the old 28% fill the band reached 3.9:1; a fill low enough to pass with `--n-6` is 4% and shows nothing; with `--n-5` a 10% fill passes everywhere). Secondary text and chrome muted stay on `--n-6`.
- Light `--color-success` is `#1e7853` (was `#1f7a55`), the one step that keeps its text 4.5:1 over a removed fill.
- The contrast test now covers text and muted over every fill on paper, band, ground and sunken, and the three status inks over every fill on paper, in both themes.
- The rail and inspector widths keep the app's shipped 200px and 400px; the token file's 184px and 360px were never what the frame drew.

Alternatives: keep `--n-6` and fade the dark fills to near nothing; lower the floor for text over a fill; take the token file's narrower frame widths (a visible layout change nobody asked for).

What would change it: a design call that the proposal layer marks with something other than a background fill, or a different dark ramp.
