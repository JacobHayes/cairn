# Graticule design language

Graticule is a design language for dense, keyboard-driven tools that people read closely: tables, queues, timelines, inspectors, small charts. It looks like a measurement workspace. Cool near-white paper panels, rows and cards sit on a plain ground, each bordered and pinned down by four small corner ticks, and the faint 24px ruling is drawn only on the canvases people gesture on (a graph, a timeline). Light and dark are both native: the rail, inspector, sheet and strip follow the theme rather than staying dark. Typography, borders and opaque surfaces carry the hierarchy. Colour is kept for state.

This document is complete on its own. `design/tokens.css`, `design/base.css` and `design/fonts/` are the reference implementation, and `design/example.html` shows the components on one page. Where they disagree with this document, this document wins.

## Principles

- **Structure over colour.** Hierarchy comes from type (uppercase grotesk titles, mono labels, tabular numbers) and hairlines (row rules, column separators, panel edges). If a screen needs colour to be readable, fix its structure.
- **Keep structure visible.** Every item (a row, a card, a panel, a menu) is an opaque surface with a border you can see against its ground in both themes, so nothing floats on the page and you never have to guess where one item ends. The contrast floors under Tokens enforce it.
- **Calm first, dense on request.** A screen shows the few things that matter at a glance and leaves the rest one hover, click or fold away (Approachable by default, below). Where someone asks for density (a table opened in full, the graph at its most detailed step), the borders, hairlines and mono labels give it a visible order.
- **Accent means state.** The electric blue is for what is selected, focused or flagged. It is never decoration, a background wash, a category colour, or the colour of ordinary links and buttons. Use it rarely enough that it still draws the eye.
- **Measured and dependable.** Square corners, crisp 1px lines, mono metadata and tabular figures. The tone is audit-ready, not playful.
- **The settled state comes first.** Every screen reads correctly with no motion. Motion only adds to it.

Avoid:
- soft rounded cards, pastel fills, gradients, illustrations or decorative motion
- a row of identical floating cards with a coloured edge on each
- removing borders or corner ticks for minimalism: they are the identity
- ruling anything but a canvas: text and rows on a ruled ground are hard to read
- always-dark chrome in the light theme, or light chrome in the dark one
- status shown by colour alone

## Approachable by default: density and progressive disclosure

Cairn is used by people who open it a few times a week as well as by people who live in it, so every screen should feel calm and friendly at first sight and get denser only when someone asks. Calm is not bare: borders and fills carry the structure, and one more fact is fine where it answers a question (a header subline, a container's progress). This section wins over any denser default elsewhere in this document.

- **Show less by default.** A row or card shows one primary thing (its title), its status, and at most one secondary fact. Everything else is one hover, click or fold away. When in doubt, hide it, and leave a count when you do ("4 not relevant hidden"), so a hidden thing is never a hidden problem.
- **More air.** A two-line list row is about 64px tall, inspector sections pad 20px, and groups sit 24px (`--space-5`) apart. Rows, controls and inspector text are 15-16px with a generous line height. Separate with space first and hairlines second.
- **Fewer shouting labels.** Mono uppercase is only for small labels and status chips. Buttons, menu items, toolbar text and tabs, inspector section headings, and helper text are sentence case in Plex Sans. At most one chip per row or card.
- **Plain words over codes.** "Due in 3 days", "3 days late" and "Decide by Oct 14", not a date and a day count run together. Sentences over abbreviations, and no bare number without a word ("Unlocks 2", not "2").
- **Quieter chrome.** A page header is one line, the title, its status and its actions, with one muted line under it for what the journey is (route and version, progress, next milestone). The rest of the context lives in the detail column. A toolbar holds the views (Graph, List, Timeline, Summary as one segmented control), the Frontier and Decisions chips, a single Filter button with a count when anything inside it is on, search, and Walk through. The status strip shows only the sync chip and "? Keys". Transient bars, such as a graph's trace bar, are one short line.
- **Detail on demand.** The inspector leads with its header, one plain sentence, and the primary action or form. The sections below are folded rows showing their name and, where useful, a count ("History", "Notes 3"), never a key-value summary.
- **Graph cards** at reading zoom show the kind and status chip, the title, and one foot line. A body line appears only where it answers something at a glance, such as a decided decision's answer.

## Tokens

Use tokens everywhere. Screen code never uses raw hex values, pixel radii above `--radius-lg`, or its own durations.

### The neutral ramp and its roles

Every neutral comes from one cool ramp, `--n-0` to `--n-15`: OKLCH hue 250 throughout, lightness stepping from 0.985 to 0.14, chroma 0.004 at the palest rising to 0.032 at mid and settling at 0.015 at the darkest. A role is a step, never a hex of its own, and a theme only chooses which step each role takes. New neutrals are added by picking a step.

| Step | Hex | L | Step | Hex | L |
| --- | --- | --- | --- | --- | --- |
| `--n-0` | `#f8fafd` | .985 | `--n-8` | `#4b5a6a` | .46 |
| `--n-1` | `#f0f4f8` | .965 | `--n-9` | `#353e48` | .36 |
| `--n-2` | `#e6ecf2` | .94 | `--n-10` | `#262f38` | .30 |
| `--n-3` | `#dbe2e9` | .91 | `--n-11` | `#1e262e` | .265 |
| `--n-4` | `#ced5dd` | .87 | `--n-12` | `#171e25` | .23 |
| `--n-5` | `#b5bfc9` | .80 | `--n-13` | `#10171e` | .20 |
| `--n-6` | `#94a0ad` | .70 | `--n-14` | `#0a1016` | .17 |
| `--n-7` | `#6d7c8c` | .58 | `--n-15` | `#050a0f` | .14 |

| Role | Token | Light | Dark |
| --- | --- | --- | --- |
| chrome (rail, inspector column, sheet, strip, toast, selection bar) | `--chrome-bg` | `--n-4` | `--n-14` |
| workspace ground, canvas ground | `--color-ground` | `--n-2` | `--n-13` |
| inside container frames | `--color-sunken` | `--n-1` | `--n-12` |
| paper: rows, cards, panels, menus | `--color-surface` | `--n-0` | `--n-11` |
| header band, table headers | `--color-band` | `--n-3` | `--n-10` |
| hairlines, row rules, separators | `--color-border` | `--n-4` | `--n-9` |
| card, row and panel borders | `--color-edge` | `--n-6` | `--n-8` |
| container frames, menu borders | `--color-frame` | `--n-7` | `--n-7` |
| hairlines on chrome | `--chrome-border` | `--n-5` | `--n-11` |
| text on paper, text on chrome | `--color-text`, `--chrome-ink` | `--n-15` | `--n-1` |
| secondary and muted text, on chrome too | `--color-secondary`, `--color-muted`, `--chrome-muted` | `--n-8` | `--n-6` |
| primary button, chart ink | `--color-primary` | `--n-12` | `--n-11` (`--ink-fill` is chrome ink) |

The hierarchy is the order of the steps. In light, paper is the lightest, then the sunken frames, the ground, the band, and the chrome the deepest: the chrome is the slightly deeper tone that frames the page. In dark the order flips: the chrome is the deepest step, then the ground, the sunken frames, paper (raised), the band, the hairlines. Chrome is always one clear step off the ground (two ramp steps), in both themes. The ramp has no gradients and no second hue, and the roles above are the only places it is used for structure.

### Core (light, canonical)

| Token | Value | Role |
| --- | --- | --- |
| `--color-primary` | `--n-12` | graphite: primary buttons, chart ink |
| `--color-secondary` | `--n-8` | steel: labels, column headers, metadata, second chart series |
| `--color-accent` | `#2f6fff` | electric blue: selection, focus ring, flagged datum |
| `--color-surface` | `--n-0` | paper: rows, cards, panels, menus |
| `--color-text` | `--n-15` | ink on paper |
| `--color-muted` | `--n-8` | secondary text (AA on the header band) |
| `--color-border` | `--n-4` | hairlines, row rules, separators, chart gridlines |
| `--color-error` | `#b8342c` | 4.5:1 on the light ground (dark: `#c53b32`) |
| `--color-success` | `#1f7a55` | |
| `--color-warning` | `#a66a12` | |
| `--color-info` | `#2f6fff` | |

### Derived

| Token | Light | Dark | Role |
| --- | --- | --- | --- |
| `--color-ground` | `--n-2` | `--n-13` | the plain ground behind workspace content, and under a canvas's ruling |
| `--color-sunken` | `--n-1` | `--n-12` | inside container frames, so the paper cards in them stand out |
| `--color-edge` | `--n-6` | `--n-8` | card, row and panel borders |
| `--color-frame` | `--n-7` | `--n-7` | container frames and menu borders |
| `--chrome-bg` | `--n-4` | `--n-14` | rail, inspector column, sheet, strip, toast, selection bar |
| `--chrome-ink` | `--n-15` | `--n-1` | text on chrome |
| `--chrome-muted` | `--n-8` | `--n-6` | secondary text on chrome |
| `--chrome-border` | `--n-5` | `--n-11` | hairlines on chrome |
| `--chrome-brand` | `#214f7c` | `#90bce9` | ink-blue (the ramp's hue at higher chroma): the brand mark and the active nav marker. Quiet identity, never state: the electric accent stays for selection and focus |
| `--color-band` | `--n-3` | `--n-10` | steel header band on panels and table headers |
| `--color-rule` | `rgba(20,24,30,.045)` | `rgba(255,255,255,.06)` | the 24px ruling, drawn by `--ruling` on canvases only |
| `--color-tick` | `--color-secondary` | `--n-6` | corner registration ticks |
| `--color-hover` | `rgba(20,24,30,.07)` | `rgba(255,255,255,.07)` | neutral row hover |
| `--color-tint` | `rgba(47,111,255,.12)` | `rgba(47,111,255,.16)` | faint blue behind a selected row |
| `--color-add` / `--color-del` | success / error at 12% | at 28% | diff line backgrounds |
| `--chart-ink` / `--chart-steel` / `--chart-grid` | primary / secondary / border | text / muted / border | chart marks and scaffold |
| `--ink-fill` / `--ink-hover` | primary / `--n-10` | chrome-ink / chrome-muted | primary button fill |

### Contrast floors

Both themes meet these, and `web/app/src/ui/tokens.test.ts` parses `design/tokens.css` and fails a token edit that breaks one:

| Pair | Floor |
| --- | --- |
| text and muted on surface and on band, muted on ground, and chrome ink, muted and brand on chrome | 4.5:1 |
| `--color-frame` on ground and on sunken | 3:1 |
| `--color-edge` on ground and on surface | 2:1 |
| surface on ground (a visible step before any border) | 1.08:1 |

### Dark theme

Dark mode follows the OS preference and can be forced with `data-theme="dark"` or `"light"` on the root element. Every theme-dependent token has a value in each theme (the tables above): the same ramp, with each role taking a darker step. The paper is raised (`--n-11`) above the ground and the chrome goes deeper than both, and the borders lighten so they keep their contrast. The accent and status colours are the same in both themes.

### Categorical and ramp palettes

Most screens need no categorical colour. When one is unavoidable (colour by group in a graph or chart), use the muted family `--cat-1` to `--cat-8`: eight hues at about 40% saturation, with lightness matched to the neutrals. It has no hue between 200 and 240, so nothing reads as the accent.

| | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| hue, sat | 96, 30% | 28, 40% | 152, 40% | 352, 40% | 268, 40% | 46, 40% | 182, 40% | 320, 40% |
| light L | 38% | 48% | 34% | 48% | 54% | 40% | 34% | 42% |
| dark L | 56% | 62% | 52% | 64% | 70% | 56% | 50% | 62% |

For ordered values, use `--ramp-1` to `--ramp-4`, four steps of the neutral ramp from dark to pale: `--n-12 --n-8 --n-6 --n-4` (dark: `--n-1 --n-6 --n-7 --n-9`). Show binary or ordinal attributes with shape, a ring or a label, not with more saturated colour.

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
| Rows, controls, inspector text | IBM Plex Sans | 15px | sentence case; generous line height; inspector section headings are 15px / 600, and its meta lines 13px mono |
| Node title in the inspector | Space Grotesk | 1.25rem / 700 | the h2 face and size in **sentence case**: a node title is the author's prose, often a full question |
| Graph node card | IBM Plex Sans title, Plex Mono kind and dates | title 15px / 600, at most 2 lines; kind as the 12px label; dates 12.5px mono | a card is a dense instrument read at varying zoom, so its title sits below the 17px card text |
| Numbers, hashes, ids, paths, emails, code | IBM Plex Mono | 12.5-13px | `font-variant-numeric: tabular-nums`; keep their case |
| Big figure (stat) | IBM Plex Mono | 1.563rem / 600 | tabular |

Mono uppercase is kept for labels, column headers, keys and status chips; buttons, menu items, toolbar text and tabs are sentence case (Approachable by default). The type scale ratio is 1.25 on a 17px base. Titles are sized for a tool, not a landing page. Fonts are self-hosted (`design/fonts/`, woff2, latin and latin-ext, SIL OFL 1.1) and nothing loads from a CDN at runtime. Fall back to `ui-sans-serif, system-ui` and `ui-monospace`.

## Layout

- **Shell.** Three zones and a strip:
  - a fixed rail on the left (brand, nav, small counts in mono) on `--chrome-bg`
  - a wide workspace in the centre on the plain `--color-ground`, with paper rows, cards and panels on it
  - an optional inspector on `--chrome-bg` for the detail of whatever is selected
  - a status strip, 28px tall, along the bottom of the workspace and inspector, holding the sync chip at its right end and "? Keys"
  The chrome follows the theme: light chrome in light, dark in dark. The workspace runs edge to edge between the rail and the viewport. Only a canvas inside it is ruled.
- **One scroller per region.** At 720px and wider the frame fills the viewport (`100dvh`) and the document never scrolls. The rail, the workspace body, the inspector body, a sheet body, a popover body and a modal body are the only scrolling elements, each scrolling once. A gesture surface (a graph or timeline canvas, which owns the wheel and touch) never sits inside a scroller. A primary table is its region's scroller, so its sticky header sticks to the right box; secondary tables (inside the inspector or a card) never scroll and end in "Show all" instead.
- **Inspector.** A right-hand column, or a resizable sheet along the bottom when the workspace must keep its full width. Both are chrome, with paper cards inside. One control hides it: the head's "Hide inspector" button (a `›|` chevron, tooltip "Hide the inspector  ]"), with no close button, since deselecting is the canvas background click or Esc. Hidden, the column folds to a 32px rail on the right edge, still visible, with a `|‹` "Show inspector" button and a vertical mono INSPECTOR label; the whole rail opens it, and so does `]`, which toggles it. Selecting a node (a canvas click or a list row) brings a hidden inspector back. The sheet has the same single control, and hidden it is a slim handle bar along the bottom that opens it. The status strip carries no inspector control.
- **Grid.** Inside the workspace, modules sit on repeating `minmax(0, 1fr)` columns.
- **Responsive.** From 720 to 1099px the inspector becomes a bottom sheet over the workspace (a drag handle, two snap heights, Esc closes it) rather than dropping below the workspace, so the document still never scrolls. Below 720px everything stacks into one scrolling column and the rail becomes a top bar with wrapping nav; a gesture surface there is an inert preview (a fitted, static picture a swipe scrolls past) with one button that opens it full screen, where gestures belong to it. Supported from about 390px to 2560px.

## Components

- **Panel / card.** Opaque paper surface, four corner ticks (L-shaped 1px strokes, 8px long, inset 3px), a 1px border, `--shadow-sm`, square corners. No ruling: only a canvas is ruled (see Charts and graphs). A panel's first label becomes its header band: `--color-band`, a bottom hairline, and an uppercase mono label in steel. Cards have a header band and an optional action row with a top hairline. Never give one edge a colour. Inspector sections are the exception: each is a sentence-case Plex Sans heading row, folded by default, showing its name and at most a count.
- **Label.** Uppercase Plex Mono 12px, letter-spacing .08em, steel. Use it for module titles, field labels above controls, and keys in key/value tables.
- **Table.** The one row component for every list of nodes (the Plan list, the frontier, folds, Mine, decision lists): opaque paper rows with a `--color-edge` hairline under each, the table inside a bordered panel on the ground, a status chip column and an owner column. Rows have room (a two-line row about 64px, a one-line row 8px 12px padding); vertical hairlines between columns only in data tables opened in full. The header is a sticky band with steel labels, uppercase mono in a data table and sentence case in the Plan list, where a column that sorts is a button. Numbers are right-aligned mono with tabular figures, and secondary parts of a cell (a percentage, a rank) are steel. Hover is a neutral tint. A tree is a classic tree: for each ancestor with more children below, a continuous 1px `--color-border` vertical line through the full row height (across the row hairlines) centred on that ancestor's chevron, and for the row itself a `├` or, as the last child, a `└` joining the line to its chevron or bullet. A container's fold is a real 24px icon button with a 12px chevron in `--color-text`; a leaf has a small bullet in the same slot, so every title starts at its level's 24px column. A container row (a group or a stage) is a `--color-band` row with a 600-weight title and its progress, so it reads as the header of the rows under it. A tree keeps the levels a person opened or folded when the row set changes (a filter, Show not relevant). A primary table is its region's scroller, so its sticky header sticks to the right box; secondary tables (inside the inspector or a card) never scroll and end in "Show all" instead.
- **Inspector.** A right-hand column, or a resizable sheet along the bottom when the workspace must keep its full width. Both are chrome, with paper cards inside.
- **Grid.** Inside the workspace, modules sit on repeating `minmax(0, 1fr)` columns.
- **Responsive.** From 720 to 1099px the inspector becomes a bottom sheet over the workspace (a drag handle, two snap heights, Esc closes it) rather than dropping below the workspace, so the document still never scrolls. Below 720px everything stacks into one scrolling column and the rail becomes a top bar with wrapping nav; a gesture surface there is an inert preview (a fitted, static picture a swipe scrolls past) with one button that opens it full screen, where gestures belong to it. Supported from about 390px to 2560px.

## Components

- **Panel / card.** Opaque paper surface, four corner ticks (L-shaped 1px strokes, 8px long, inset 3px), a 1px border, `--shadow-sm`, square corners. No ruling: only a canvas is ruled (see Charts and graphs). A panel's first label becomes its header band: `--color-band`, a bottom hairline, and an uppercase mono label in steel. Cards have a header band and an optional action row with a top hairline. Never give one edge a colour. Inspector sections are the exception: each is a sentence-case Plex Sans heading row, folded by default, showing its name and at most a count.
- **Label.** Uppercase Plex Mono 12px, letter-spacing .08em, steel. Use it for module titles, field labels above controls, and keys in key/value tables.
- **Table.** The one row component for every list of nodes (the Plan list, the frontier, folds, Mine, decision lists): opaque paper rows with a `--color-edge` hairline under each, the table inside a bordered panel on the ground, a status chip column and an owner column. Rows have room (a two-line row about 64px, a one-line row 8px 12px padding); vertical hairlines between columns only in data tables opened in full. The header is a sticky band with steel labels, uppercase mono in a data table and sentence case in the Plan list, where a column that sorts is a button. Numbers are right-aligned mono with tabular figures, and secondary parts of a cell (a percentage, a rank) are steel. Hover is a neutral tint. In a tree each depth level draws a 1px `--color-edge` guide line through the indent gutter of its descendants (24px per level), the fold chevron sits on the guide, and a container row (a group or a stage) is a `--color-band` row with a 600-weight title and its progress, so it reads as the header of the rows under it. A primary table is its region's scroller and has no max-height of its own; a secondary table never scrolls.
- **Selection.** A selected row, card or list item gets a 2px accent line on its left edge (an inset box-shadow) and the faint blue tint. The keyboard focus ring is a 2px accent outline on every interactive element. Selection and focus are the main uses of the accent.
- **Status.** An uppercase mono label in a hairline chip, with a 6px dot: success (done or approved), error (failed or rejected), warning (deferred or attention), steel (pending), hollow (superseded), and ink, filled for actionable now and a ring for in progress. The text is always there; the dot only backs it up. No two states share a dot shape: the blocked dot is a 6px square and a skipped dot is hollow and struck through, so it differs from a hollow superseded or not-relevant one. Where the word cannot fit (a graph card at far zoom, a narrow timeline row), a glyph whose shape names the state stands in for the chip, and a hover gives the word.
- **Buttons.** Hairline box, paper fill, Plex Sans 14px, `--radius-md`. Hover darkens the border to steel. A primary button is filled with graphite ink (light rail ink in dark mode), never blue. Ghost buttons are transparent. Disabled buttons are at 45% opacity.
- **Inputs and selects.** A hairline box at `--control-h`, body type, and a hairline that turns steel on hover. On focus the border turns to the accent. A select is square with a drawn steel chevron, and every select in the product shares that one style. Checkboxes, radios and ranges take `accent-color: var(--color-text)`, not blue. Number, date and textarea inputs use mono.
- **Links.** Inherit the text colour, with a 1px underline in `--color-border` that turns to `currentColor` on hover.
- **Keyboard hints.** A `kbd` chip: chrome ink on chrome with a hairline border. The status strip shows only "? Keys"; `?` opens a modal key sheet (a key-value table of `kbd` chips) for the current screen.
- **Toast.** A chrome chip at the bottom right of the workspace, 12px above the status strip (above a sheet when one is open), so it never covers the sync chip. Mono 13px, with `--shadow-md`. It rises in over 180ms, leaves after 2.5s, pauses on hover, has no close button, and a new one replaces the old. Saves never raise a toast; the sync chip reports them.
- **Status strip and sync chip.** The strip is chrome and 28px tall. The sync chip is one button at its right end: a 6px dot and an always-visible mono label in chrome ink, a one-line sentence on hover, and a popover on click. Its colour says what it asks of you: green, nothing; steel, working on it; amber, safe to wait (it will sort itself out); red, your change is not landing until you act. The label and dot shape tell its states apart, so colour is never the only signal.
- **Segmented control.** A hairline row of segments at `--control-h`, sentence case. The active segment gets the band fill and a 2px ink underline (the active-nav marker), not the accent: it shows where you are, not a selection. The projection switcher is one, always at the right end of its toolbar.
- **Popover and menu.** A paper panel with a 1px `--color-frame` border and `--shadow-md`, anchored to its control (a context menu opens at the pointer), its body the one scroller (capped at `min(60dvh, 480px)`), closed by Esc. Menu items are sentence case, 14px, padded 4px 12px, with a neutral hover and an optional right-aligned mono key hint. Items are grouped by how often they are used, with a hairline between groups and an optional mono group label above one.
- **Option list.** A choice among a few answers (a decision's choices, a radio group): a bordered stack with a hairline between options. The chosen option is banded (`--color-band`) with an inset 1px ink ring, and what picking an option does is right-aligned in mono steel.
- **Group frame.** A container drawn around its members: a 1px `--color-frame` border, a header row (`--color-band` with a bottom hairline) holding the kind label, the title, progress and its "4/10" count, the status chip and the fold control, and a sunken body (`--color-sunken`, opaque) so the paper cards inside stand out.
- **Tooltip.** A jargon term (Frontier, Gravity) carries a native `title` on its primary element that says what it means in one sentence. There are no first-run hints or custom tooltips.
- **Bottom sheet.** A chrome sheet with a 24px drag handle (a short hairline bar), snapping to two heights; its body is the one scroller.
- **Selection bar.** A chrome bar floating bottom-centre over the workspace, above the strip, while several items are selected: the count, the bulk actions, and Clear.
- **Rank tag.** A small square hanging off a card's top-left corner: the primary-button pair (`--ink-fill` with `--color-surface` mono figures), so it flips with the theme.
- **Hanging tag.** A small mono label hanging off a card's top-right or bottom-right edge for transient or lens information (a trace role, a signal value). Each slot has one owner, so two tags never displace each other.
- **Dates.** Words first: `Decide by Oct 14`, `Due in 3 days`, `3 days late`. A date is `Oct 14` in the current year and `Oct 14, 2027` otherwise; ISO dates appear only in tooltips and History. A soon date is in warning ink and an overdue one in error ink, always with its words. Dates in a column or on an axis are mono with tabular figures.
- **Diff and log blocks.** Mono, hairline border. Added and removed lines get `--color-add` and `--color-del`, and hunk headers are steel.

## Charts and graphs

- **The ruling lives on canvases.** The 24px ruling (`--ruling`) on `--color-ground`, with corner ticks, is drawn by the graph canvas, the timeline and the phone map preview, where it reads as a measuring surface. Content never sits on it anywhere else.
- A faint scaffold: gridlines in `--chart-grid` (crisp 1px) and a steel axis. Axis text is mono 11px in steel.
- Marks are graphite (`--chart-ink`), with a second series in steel (`--chart-steel`). Lines are 1.5px.
- Graph cards have a `--color-edge` border and keep their ticks. Zoomed out, a card's title takes the whole card and wraps to the lines that fit, set at 14px on screen or the largest size at which the whole title shows, whichever is less (a title of 40 characters shows whole at mid zoom); a title is never cut to the reading-zoom line count.
- A selected or anomalous mark is the accent, and a threshold or reference value is a dashed accent line (1.5px, dash 4 3).
- Heat scales run from steel to blue. Categorical colour uses the `--cat-*` family above, and the accent is never one of its hues.
- In graphs, edges are 1px hairlines: steel while they still wait and border grey once satisfied, so the steel lines are the blockers that remain. An edge's kind is told by its line style (solid or dotted) and its end marker (an arrowhead, a hollow diamond, a bar), never by a text label on the line; a hover gives its sentence. Lines keep their width at every zoom.
- Selecting a node traces it: its downstream ("unblocks") is drawn in the accent and its upstream ("needs") in ink, each traced node carries a NEEDS or UNBLOCKS tag so colour is never the only signal, and everything else fades to one fixed level (fades never multiply). Hovering an edge draws it and its two ends in ink.
- An ordinal signal on graph nodes (a rank, a weight) is a single labelled chip tinted on the neutral ramp, never a border weight or an opacity.

## Accessibility

- Ink on paper and text on chrome both meet WCAG AA: 4.5:1 for small text, including muted text on the header band (held by the contrast test).
- Prose is at least 17px, and rows, controls and inspector text at least 15px.
- Every interactive element shows a visible 2px focus ring.
- State is never shown by colour alone: status has a label, and selection has a line as well as the tint.
- Motion is additive and fully off under `prefers-reduced-motion`.

## Appendix: provenance

Graticule comes from the Katagami design library ("Graticule", https://katagami.ai/language/en-019efa7c-5133-7f82-a6af-bce556fd39fd, which needs sign-in). It was first adapted for a local analyst workbench. Its core colour, type, spacing, radius, shadow and motion tokens are unchanged. This version differs from the library entry in these ways:

- **Accent use.** The library allows exactly one blue element per view. Here the accent marks state (selection, focus, flagged data) without a hard count.
- **Contrast.** `--color-muted` is `#616b7a`, not `#6b7585` (3.9:1 on the band), and dark-mode steel is `#a3acba`.
- **Dark theme.** The library defines light panels only. The dark values here are an addition, and chrome follows the theme rather than staying graphite.
- **Additions.** The library has none of these: the rail, chrome, ground, sunken, edge, frame, band, ruling, tick, hover, tint and diff tokens; `--control-h`; the categorical and ramp palettes; and the select, keyboard-hint and toast styles.
- **Layout.** The library's inspector is always a right column, and it puts graphite margins around the panels. Here the workspace runs edge to edge on a plain ground, chrome follows the theme, the ruling is only on canvases, and the inspector may be a bottom sheet.
- **Headings and imagery.** The library calls for oversized headlines, a full-viewport schematic hero, and orthographic engineering illustrations (a surveyor's theodolite) in hairline graphite with one blue element. This version is for tools, so titles are modest and there is no hero or imagery.
- **Fonts** are self-hosted rather than loaded from Google Fonts.
- **Density.** The library is dense throughout. This version shows less by default and roomier rows (Approachable by default), keeps the ruling (on canvases), ticks and borders, and adds the status strip, sync chip, segmented control, popover, bottom sheet, selection bar, and rank and hanging tags.
- **Chrome and ruling.** Chrome follows the theme, and the ruling is only on canvases, so content never sits on a grid.
