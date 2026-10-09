# Graticule design language

Graticule is a design language for dense, keyboard-driven tools that people read closely: tables, queues, timelines, inspectors, small charts. It looks like a measurement workspace. Cool near-white paper panels sit on a deep graphite table, a faint 24px ruling runs through every panel, and each panel is pinned down by four small corner ticks. Typography and hairline structure carry the hierarchy. Colour is kept for state.

This document is complete on its own. `design/tokens.css`, `design/base.css` and `design/fonts/` are the reference implementation, and `design/example.html` shows the components on one page. Where they disagree with this document, this document wins.

## Principles

- **Structure over colour.** Hierarchy comes from type (uppercase grotesk titles, mono labels, tabular numbers) and hairlines (row rules, column separators, panel edges). If a screen needs colour to be readable, fix its structure.
- **Dense, but organised.** Rows are compact and spacing is tight. The ruling, the hairlines and the mono labels give dense areas a visible order, so density looks deliberate.
- **Accent means state.** The electric blue is for what is selected, focused or flagged. It is never decoration, a background wash, a category colour, or the colour of ordinary links and buttons. Use it rarely enough that it still draws the eye.
- **Measured and dependable.** Square corners, crisp 1px lines, mono metadata and tabular figures. The tone is audit-ready, not playful.
- **The settled state comes first.** Every screen reads correctly with no motion. Motion only adds to it.

Avoid:
- soft rounded cards, pastel fills, gradients, illustrations or decorative motion
- a row of identical floating cards with a coloured edge on each
- removing the ruling or corner ticks for minimalism: they are the identity
- status shown by colour alone

## Tokens

Use tokens everywhere. Screen code never uses raw hex values, pixel radii above `--radius-lg`, or its own durations.

### Core (light, canonical)

| Token | Value | Role |
| --- | --- | --- |
| `--color-primary` | `#20242b` | graphite: rails, inspector, primary buttons, chart ink |
| `--color-secondary` | `#5e6877` | steel: labels, column headers, metadata, second chart series |
| `--color-accent` | `#2f6fff` | electric blue: selection, focus ring, flagged datum |
| `--color-background` | `#171b22` | the drafting-table ground behind everything |
| `--color-surface` | `#f7f9fc` | paper panels |
| `--color-text` | `#14181e` | ink on paper |
| `--color-muted` | `#616b7a` | secondary text (AA on the header band) |
| `--color-border` | `#c3ccd8` | hairlines, row rules, separators, chart gridlines |
| `--color-error` | `#c53b32` | |
| `--color-success` | `#1f7a55` | |
| `--color-warning` | `#a66a12` | |
| `--color-info` | `#2f6fff` | |

### Derived

| Token | Light | Dark | Role |
| --- | --- | --- | --- |
| `--rail-ink` | `#e6ebf2` | same | text on graphite |
| `--rail-muted` | `#97a1b0` | same | secondary text on graphite |
| `--rail-border` | `#343b46` | same | hairlines on graphite |
| `--color-band` | `#e8ecf2` | `#282d35` | steel header band on panels and table headers |
| `--color-rule` | `rgba(20,24,30,.045)` | `rgba(255,255,255,.06)` | the 24px ruling |
| `--color-tick` | `--color-secondary` | `#97a1b0` | corner registration ticks |
| `--color-hover` | `rgba(20,24,30,.045)` | `rgba(255,255,255,.04)` | neutral row hover |
| `--color-tint` | `rgba(47,111,255,.08)` | `rgba(47,111,255,.10)` | faint blue behind a selected row |
| `--color-add` / `--color-del` | success / error at 12% | at 28% | diff line backgrounds |
| `--chart-ink` / `--chart-steel` / `--chart-grid` | primary / secondary / border | text / muted / border | chart marks and scaffold |
| `--ink-fill` / `--ink-hover` | primary / rail-border | rail-ink / rail-muted | primary button fill |

### Dark theme

Dark mode follows the OS preference and can be forced with `data-theme="dark"` or `"light"` on the root element. Panels turn graphite and everything else stays put:

- `--color-surface: #20242b`, `--color-text: #e6ebf2`, `--color-muted: #97a1b0`, `--color-border: #343b46`
- `--color-secondary: #a3acba` (the light steel is only 2.5:1 on graphite)
- the derived values in the table above

The accent, status colours, ground, rails and every rule are the same in both themes.

### Categorical and ramp palettes

Most screens need no categorical colour. When one is unavoidable (colour by group in a graph or chart), use the muted family `--cat-1` to `--cat-8`: eight hues at about 40% saturation, with lightness matched to the neutrals. It has no hue between 200 and 240, so nothing reads as the accent.

| | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| hue, sat | 96, 30% | 28, 40% | 152, 40% | 352, 40% | 268, 40% | 46, 40% | 182, 40% | 320, 40% |
| light L | 38% | 48% | 34% | 48% | 54% | 40% | 34% | 42% |
| dark L | 56% | 62% | 52% | 64% | 70% | 56% | 50% | 62% |

For ordered values, use the neutral ramp `--ramp-1` to `--ramp-4`, from dark to pale: `#20242b #5e6877 #97a1b0 #c3ccd8` (dark: `#e6ebf2 #97a1b0 #6b7585 #454d59`). Show binary or ordinal attributes with shape, a ring or a label, not with more saturated colour.

### Shape, space, depth, motion

- Radii: `--radius-none: 0`, `--radius-sm: 0`, `--radius-md: 2px` (buttons, inputs), `--radius-lg: 4px` (largest allowed), `--radius-full: 9999px` (status dots only). Panels and cards are square.
- Spacing (8px base): `--space-1..8` = 4, 8, 12, 16, 24, 32, 48, 64px. Use small steps inside a module and large ones between modules.
- Borders: 1px solid hairlines. 2px only for the accent line on a selected item and for the active nav marker.
- Shadows: `--shadow-sm: 0 1px 0 rgba(20,24,30,.06)` on panels, `--shadow-md: 0 12px 30px rgba(20,24,30,.10)` on floating layers (toasts, popovers), `--shadow-lg: 0 24px 60px rgba(20,24,30,.14)` on modals.
- Controls: `--control-h: 32px`. Inputs, selects and buttons in a row share one height.
- Motion: `--motion-duration: 180ms`, `--motion-easing: cubic-bezier(0.2, 0.7, 0, 1)`. Only small additions (a drawer slides in, a row highlight fades, a toast rises). Turn all of it off under `prefers-reduced-motion`.

## Typography

| Role | Face | Size / weight | Notes |
| --- | --- | --- | --- |
| Page title (h1) | Space Grotesk | 1.563rem / 700 | uppercase, letter-spacing .02em, line-height 1.2 |
| Section title (h2) | Space Grotesk | 1.25rem / 700 | uppercase |
| Sub-label (h3), module label, column header, key | IBM Plex Mono | 12-13px / 500 | uppercase, letter-spacing .06-.08em, steel |
| Prose and card text | IBM Plex Sans | 17px / 400 | line-height 1.5, letter-spacing .01em |
| Table rows, controls | IBM Plex Sans | 14-14.5px | |
| Numbers, hashes, ids, paths, emails, code | IBM Plex Mono | 12.5-13px | `font-variant-numeric: tabular-nums`; keep their case |
| Big figure (stat) | IBM Plex Mono | 1.563rem / 600 | tabular |

The type scale ratio is 1.25 on a 17px base. Titles are sized for a tool, not a landing page. Fonts are self-hosted (`design/fonts/`, woff2, latin and latin-ext, SIL OFL 1.1) and nothing loads from a CDN at runtime. Fall back to `ui-sans-serif, system-ui` and `ui-monospace`.

## Layout

- **Shell.** Three zones on the graphite ground:
  - a fixed graphite rail on the left (brand, nav, small counts in mono)
  - a wide paper workspace in the centre
  - an optional graphite inspector for the detail of whatever is selected
  The workspace is the paper itself, ruled and ticked, running edge to edge between the rail and the viewport.
- **Inspector.** A right-hand column, or a resizable sheet along the bottom when the workspace must keep its full width. Both are graphite, with paper cards inside.
- **Grid.** Inside the workspace, modules sit on repeating `minmax(0, 1fr)` columns.
- **Responsive.** Below 1100px the inspector drops below the workspace. Below 720px everything stacks into one scrolling column and the rail becomes a top bar with wrapping nav. Supported from about 390px to 2560px.

## Components

- **Panel / card.** Paper surface, the 24px ruling, four corner ticks (L-shaped 1px strokes, 8px long, inset 3px), a 1px border, `--shadow-sm`, square corners. A panel's first label becomes its header band: `--color-band`, a bottom hairline, and an uppercase mono label in steel. Cards have a header band and an optional action row with a top hairline. Never give one edge a colour.
- **Label.** Uppercase Plex Mono 12px, letter-spacing .08em, steel. Use it for module titles, field labels above controls, and keys in key/value tables.
- **Table.** Compact rows (4px 8px padding) with a hairline under each row and vertical hairlines between columns. The header is a sticky band with uppercase mono steel labels. Numbers are right-aligned mono with tabular figures, and secondary parts of a cell (a percentage, a rank) are steel. Hover is a neutral tint. Wrap long tables in a bordered scroll container.
- **Selection.** A selected row, card or list item gets a 2px accent line on its left edge (an inset box-shadow) and the faint blue tint. The keyboard focus ring is a 2px accent outline on every interactive element. Selection and focus are the main uses of the accent.
- **Status.** An uppercase mono label in a hairline chip, with a 6px dot: success (done or approved), error (failed or rejected), warning (deferred or attention), steel (pending), hollow (superseded). The text is always there; the dot only backs it up.
- **Buttons.** Hairline box, paper fill, Plex Sans 14px, `--radius-md`. Hover darkens the border to steel. A primary button is filled with graphite ink (light rail ink in dark mode), never blue. Ghost buttons are transparent. Disabled buttons are at 45% opacity.
- **Inputs and selects.** A hairline box at `--control-h`, body type, and a hairline that turns steel on hover. On focus the border turns to the accent. A select is square with a drawn steel chevron, and every select in the product shares that one style. Checkboxes, radios and ranges take `accent-color: var(--color-text)`, not blue. Number, date and textarea inputs use mono.
- **Links.** Inherit the text colour, with a 1px underline in `--color-border` that turns to `currentColor` on hover.
- **Keyboard hints.** A mono bar or `kbd` chip: rail-muted text on graphite with a hairline border. Show the shortcuts for the current screen.
- **Toast.** A graphite chip at the bottom right, mono 13px, with `--shadow-md`. It rises in over 180ms.
- **Diff and log blocks.** Mono, hairline border. Added and removed lines get `--color-add` and `--color-del`, and hunk headers are steel.

## Charts

- A faint scaffold: gridlines in `--chart-grid` (crisp 1px) and a steel axis. Axis text is mono 11px in steel.
- Marks are graphite (`--chart-ink`), with a second series in steel (`--chart-steel`). Lines are 1.5px.
- A selected or anomalous mark is the accent, and a threshold or reference value is a dashed accent line (1.5px, dash 4 3).
- Heat scales run from steel to blue. Categorical colour uses the `--cat-*` family above, and the accent is never one of its hues.
- In graphs, edges are border-grey hairlines whose opacity follows weight. Hovering or selecting a node turns it and its edges blue, and other nodes fade toward the paper colour.

## Accessibility

- Ink on paper and text on graphite both meet WCAG AA: 4.5:1 for small text, including muted text on the header band.
- Body text is at least 17px and table rows at least 14.5px.
- Every interactive element shows a visible 2px focus ring.
- State is never shown by colour alone: status has a label, and selection has a line as well as the tint.
- Motion is additive and fully off under `prefers-reduced-motion`.

## Appendix: provenance

Graticule comes from the Katagami design library ("Graticule", https://katagami.ai/language/en-019efa7c-5133-7f82-a6af-bce556fd39fd, which needs sign-in). It was first adapted for a local analyst workbench. Its core colour, type, spacing, radius, shadow and motion tokens are unchanged. This version differs from the library entry in these ways:

- **Accent use.** The library allows exactly one blue element per view. Here the accent marks state (selection, focus, flagged data) without a hard count.
- **Contrast.** `--color-muted` is `#616b7a`, not `#6b7585` (3.9:1 on the band), and dark-mode steel is `#a3acba`.
- **Dark theme.** The library defines light panels only. The dark panel values here are an addition.
- **Additions.** The library has none of these: the rail, band, ruling, tick, hover, tint and diff tokens; `--control-h`; the categorical and ramp palettes; and the select, keyboard-hint and toast styles.
- **Layout.** The library's inspector is always a right column, and it puts graphite margins around the panels. Here the workspace paper runs edge to edge, and the inspector may be a bottom sheet.
- **Headings and imagery.** The library calls for oversized headlines, a full-viewport schematic hero, and orthographic engineering illustrations (a surveyor's theodolite) in hairline graphite with one blue element. This version is for tools, so titles are modest and there is no hero or imagery.
- **Fonts** are self-hosted rather than loaded from Google Fonts.
