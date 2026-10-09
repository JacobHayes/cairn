# Graticule design language

Graticule is a design language for dense, keyboard-driven tools that people read closely: tables, queues, timelines, inspectors, small charts. It looks like a measurement workspace. Cool near-white paper panels sit on a deep graphite table, a faint 24px ruling runs through every panel, and each panel is pinned down by four small corner ticks. Typography and hairline structure carry the hierarchy. Colour is kept for state.

This document is complete on its own. `design/tokens.css`, `design/base.css` and `design/fonts/` are the reference implementation, and `design/example.html` shows the components on one page. Where they disagree with this document, this document wins.

## Principles

- **Structure over colour.** Hierarchy comes from type (uppercase grotesk titles, mono labels, tabular numbers) and hairlines (row rules, column separators, panel edges). If a screen needs colour to be readable, fix its structure.
- **Calm first, dense on request.** A screen shows the few things that matter at a glance and leaves the rest one hover, click or fold away (Approachable by default, below). Where someone asks for density (a table opened in full, the graph at its most detailed step), the ruling, the hairlines and the mono labels give it a visible order.
- **Accent means state.** The electric blue is for what is selected, focused or flagged. It is never decoration, a background wash, a category colour, or the colour of ordinary links and buttons. Use it rarely enough that it still draws the eye.
- **Measured and dependable.** Square corners, crisp 1px lines, mono metadata and tabular figures. The tone is audit-ready, not playful.
- **The settled state comes first.** Every screen reads correctly with no motion. Motion only adds to it.

Avoid:
- soft rounded cards, pastel fills, gradients, illustrations or decorative motion
- a row of identical floating cards with a coloured edge on each
- removing the ruling or corner ticks for minimalism: they are the identity
- status shown by colour alone

## Approachable by default: density and progressive disclosure

Cairn is used by people who open it a few times a week as well as by people who live in it, so every screen should feel calm and friendly at first sight and get denser only when someone asks. This section wins over any denser default elsewhere in this document.

- **Show less by default.** A row or card shows one primary thing (its title), its status, and at most one secondary fact. Everything else is one hover, click or fold away. When in doubt, hide it, and leave a count when you do ("4 not relevant hidden"), so a hidden thing is never a hidden problem.
- **More air.** A two-line list row is about 64px tall, inspector sections pad 20px, and groups sit 24px (`--space-5`) apart. Rows, controls and inspector text are 15-16px with a generous line height. Separate with space first and hairlines second.
- **Fewer shouting labels.** Mono uppercase is only for small labels and status chips. Buttons, menu items, toolbar text and tabs, inspector section headings, and helper text are sentence case in Plex Sans. At most one chip per row or card.
- **Plain words over codes.** "Due in 3 days", "3 days late" and "Decide by Oct 14", not a date and a day count run together. Sentences over abbreviations, and no bare number without a word ("Unblocks 2", not "2").
- **Quieter chrome.** A page header is one line: the title and its status. Context such as a version or progress lives in the detail column. A toolbar holds the page tabs, at most two filter controls (one of them a single Filter button with a count when anything inside it is on), search, and the projection switcher. The status strip shows only the sync chip and "? Keys". Transient bars, such as a graph's trace bar, are one short line.
- **Detail on demand.** The inspector leads with its header, one plain sentence, and the primary action or form. The sections below are folded rows showing their name and, where useful, a count ("History", "Notes 3"), never a key-value summary.
- **Graph cards** at reading zoom show the kind and status chip, the title, and one foot line. A body line appears only where it answers something at a glance, such as a decided decision's answer.

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

In dark mode the inspector column, the bottom sheet and the status strip sit on the ground colour (`--color-background`) rather than graphite, because dark paper is graphite and the paper cards inside them would otherwise merge with their column.

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
| Rows, controls, inspector text | IBM Plex Sans | 15-16px | sentence case; generous line height |
| Node title in the inspector | Space Grotesk | 1.25rem / 700 | the h2 face and size in **sentence case**: a node title is the author's prose, often a full question |
| Graph node card | IBM Plex Sans title, Plex Mono kind and dates | title 15px / 600, at most 2 lines; kind as the 12px label; dates 12.5px mono | a card is a dense instrument read at varying zoom, so its title sits below the 17px card text |
| Numbers, hashes, ids, paths, emails, code | IBM Plex Mono | 12.5-13px | `font-variant-numeric: tabular-nums`; keep their case |
| Big figure (stat) | IBM Plex Mono | 1.563rem / 600 | tabular |

Mono uppercase is kept for labels, column headers, keys and status chips; buttons, menu items, toolbar text and tabs are sentence case (Approachable by default). The type scale ratio is 1.25 on a 17px base. Titles are sized for a tool, not a landing page. Fonts are self-hosted (`design/fonts/`, woff2, latin and latin-ext, SIL OFL 1.1) and nothing loads from a CDN at runtime. Fall back to `ui-sans-serif, system-ui` and `ui-monospace`.

## Layout

- **Shell.** Three zones and a strip on the graphite ground:
  - a fixed graphite rail on the left (brand, nav, small counts in mono)
  - a wide paper workspace in the centre
  - an optional graphite inspector for the detail of whatever is selected
  - a graphite status strip, 28px tall, along the bottom of the workspace and inspector, holding the sync chip at its right end and "? Keys"
  The workspace is the paper itself, ruled and ticked, running edge to edge between the rail and the viewport.
- **One scroller per region.** At 720px and wider the frame fills the viewport (`100dvh`) and the document never scrolls. The rail, the workspace body, the inspector body, a sheet body, a popover body and a modal body are the only scrolling elements, each scrolling once. A gesture surface (a graph or timeline canvas, which owns the wheel and touch) never sits inside a scroller. A primary table is its region's scroller, so its sticky header sticks to the right box; secondary tables (inside the inspector or a card) never scroll and end in "Show all" instead.
- **Inspector.** A right-hand column, or a resizable sheet along the bottom when the workspace must keep its full width. Both are graphite, with paper cards inside.
- **Grid.** Inside the workspace, modules sit on repeating `minmax(0, 1fr)` columns.
- **Responsive.** From 720 to 1099px the inspector becomes a bottom sheet over the workspace (a drag handle, two snap heights, Esc closes it) rather than dropping below the workspace, so the document still never scrolls. Below 720px everything stacks into one scrolling column and the rail becomes a top bar with wrapping nav; a gesture surface there is an inert preview (a fitted, static picture a swipe scrolls past) with one button that opens it full screen, where gestures belong to it. Supported from about 390px to 2560px.

## Components

- **Panel / card.** Paper surface, the 24px ruling, four corner ticks (L-shaped 1px strokes, 8px long, inset 3px), a 1px border, `--shadow-sm`, square corners. A panel's first label becomes its header band: `--color-band`, a bottom hairline, and an uppercase mono label in steel. Cards have a header band and an optional action row with a top hairline. Never give one edge a colour. Inspector sections are the exception: each is a sentence-case Plex Sans heading row, folded by default, showing its name and at most a count.
- **Label.** Uppercase Plex Mono 12px, letter-spacing .08em, steel. Use it for module titles, field labels above controls, and keys in key/value tables.
- **Table.** Rows with room (a two-line row about 64px, a one-line row 8px 12px padding) and a hairline under each row; vertical hairlines between columns only in data tables opened in full. The header is a sticky band with steel labels, uppercase mono in a data table and sentence case in the Plan list, where a column that sorts is a button. Numbers are right-aligned mono with tabular figures, and secondary parts of a cell (a percentage, a rank) are steel. Hover is a neutral tint. A primary table is its region's scroller and has no max-height of its own; a secondary table never scrolls.
- **Selection.** A selected row, card or list item gets a 2px accent line on its left edge (an inset box-shadow) and the faint blue tint. The keyboard focus ring is a 2px accent outline on every interactive element. Selection and focus are the main uses of the accent.
- **Status.** An uppercase mono label in a hairline chip, with a 6px dot: success (done or approved), error (failed or rejected), warning (deferred or attention), steel (pending), hollow (superseded), and ink, filled for actionable now and a ring for in progress. The text is always there; the dot only backs it up. No two states share a dot shape: the blocked dot is a 6px square and a skipped dot is hollow and struck through, so it differs from a hollow superseded or not-relevant one. Where the word cannot fit (a graph card at far zoom, a narrow timeline row), a glyph whose shape names the state stands in for the chip, and a hover gives the word.
- **Buttons.** Hairline box, paper fill, Plex Sans 14px, `--radius-md`. Hover darkens the border to steel. A primary button is filled with graphite ink (light rail ink in dark mode), never blue. Ghost buttons are transparent. Disabled buttons are at 45% opacity.
- **Inputs and selects.** A hairline box at `--control-h`, body type, and a hairline that turns steel on hover. On focus the border turns to the accent. A select is square with a drawn steel chevron, and every select in the product shares that one style. Checkboxes, radios and ranges take `accent-color: var(--color-text)`, not blue. Number, date and textarea inputs use mono.
- **Links.** Inherit the text colour, with a 1px underline in `--color-border` that turns to `currentColor` on hover.
- **Keyboard hints.** A `kbd` chip: rail-muted text on graphite with a hairline border. The status strip shows only "? Keys"; `?` opens a modal key sheet (a key-value table of `kbd` chips) for the current screen.
- **Toast.** A graphite chip at the bottom right of the workspace, 12px above the status strip (above a sheet when one is open), so it never covers the sync chip. Mono 13px, with `--shadow-md`. It rises in over 180ms, leaves after 2.5s, pauses on hover, has no close button, and a new one replaces the old. Saves never raise a toast; the sync chip reports them.
- **Status strip and sync chip.** The strip is graphite and 28px tall. The sync chip is one button at its right end: a 6px dot and an always-visible mono label in rail ink, a one-line sentence on hover, and a popover on click. Its colour says what it asks of you: green, nothing; steel, working on it; amber, safe to wait (it will sort itself out); red, your change is not landing until you act. The label and dot shape tell its states apart, so colour is never the only signal.
- **Segmented control.** A hairline row of segments at `--control-h`, sentence case. The active segment gets the band fill and a 2px ink underline (the active-nav marker), not the accent: it shows where you are, not a selection. The projection switcher is one, always at the right end of its toolbar.
- **Popover and menu.** A paper panel on `--shadow-md`, anchored to its control, its body the one scroller (capped at `min(60dvh, 480px)`), closed by Esc. Menu items are sentence case, grouped by how often they are used, with hairlines between groups.
- **Bottom sheet.** A graphite sheet with a 24px drag handle (a short hairline bar), snapping to two heights; its body is the one scroller.
- **Selection bar.** A graphite bar floating bottom-centre over the workspace, above the strip, while several items are selected: the count, the bulk actions, and Clear.
- **Rank tag.** A small square hanging off a card's top-left corner: the primary-button pair (`--ink-fill` with `--color-surface` mono figures), so it flips with the theme.
- **Hanging tag.** A small mono label hanging off a card's top-right or bottom-right edge for transient or lens information (a trace role, a signal value). Each slot has one owner, so two tags never displace each other.
- **Dates.** Words first: `Decide by Oct 14`, `Due in 3 days`, `3 days late`. A date is `Oct 14` in the current year and `Oct 14, 2027` otherwise; ISO dates appear only in tooltips and History. A soon date is in warning ink and an overdue one in error ink, always with its words. Dates in a column or on an axis are mono with tabular figures.
- **Diff and log blocks.** Mono, hairline border. Added and removed lines get `--color-add` and `--color-del`, and hunk headers are steel.

## Charts

- A faint scaffold: gridlines in `--chart-grid` (crisp 1px) and a steel axis. Axis text is mono 11px in steel.
- Marks are graphite (`--chart-ink`), with a second series in steel (`--chart-steel`). Lines are 1.5px.
- A selected or anomalous mark is the accent, and a threshold or reference value is a dashed accent line (1.5px, dash 4 3).
- Heat scales run from steel to blue. Categorical colour uses the `--cat-*` family above, and the accent is never one of its hues.
- In graphs, edges are 1px hairlines: steel while they still wait and border grey once satisfied, so the steel lines are the blockers that remain. An edge's kind is told by its line style (solid or dotted) and its end marker (an arrowhead, a hollow diamond, a bar), never by a text label on the line; a hover gives its sentence. Lines keep their width at every zoom.
- Selecting a node traces it: its downstream ("unblocks") is drawn in the accent and its upstream ("needs") in ink, each traced node carries a NEEDS or UNBLOCKS tag so colour is never the only signal, and everything else fades to one fixed level (fades never multiply). Hovering an edge draws it and its two ends in ink.
- An ordinal signal on graph nodes (a rank, a weight) is a single labelled chip tinted on the neutral ramp, never a border weight or an opacity.

## Accessibility

- Ink on paper and text on graphite both meet WCAG AA: 4.5:1 for small text, including muted text on the header band.
- Prose is at least 17px, and rows, controls and inspector text at least 15px.
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
- **Density.** The library is dense throughout. This version shows less by default and roomier rows (Approachable by default), keeps the ruling, ticks and hairlines, and adds the status strip, sync chip, segmented control, popover, bottom sheet, selection bar, and rank and hanging tags.
