# Graticule design language

Graticule is a design language for dense, keyboard-driven tools that people read closely: tables, queues, timelines, inspectors, small charts. It looks like a measurement workspace. Cool near-white paper panels, rows and cards sit on a plain ground, each bordered and pinned down by four small corner ticks, and the faint 24px ruling is drawn only on the canvases people gesture on (a graph, a timeline). Light and dark are both native: the rail, inspector and sheet follow the theme rather than staying dark. Typography, borders and opaque surfaces carry the hierarchy. Colour is kept for state.

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
- **Quieter chrome.** A page header is one line: the title, its status chip and a `⋯` menu. What the journey is (progress, next milestone, route and its upgrade) lives only on the journey card in the inspector, never repeated under the header. Edit is one state entered from `⋯`: it brings the structure tools and the name and description editor together in one band, and Done leaves it. The Assistant is not a header button or a rail item: it opens from the inspector column's tabs (see Inspector). A toolbar holds the views (Graph, List, Timeline, Summary as one segmented control), the Frontier and Decisions chips, a single Filter button with a count when anything inside it is on, search, and Walk through. The frame has no status strip: the sync dot lives at the right edge of the brand row in the rail. Transient bars are one short line.
- **Detail on demand.** The inspector leads with its header, one plain sentence, and the primary action or form. The sections below are folded rows showing their name and, where useful, a count ("History", "Notes 3"; never a "0"), never a key-value summary. A folded row is one compact line (about 44px) whose label and chevron are vertically centred, and the whole row opens it.
- **A journey opens as the stock view, and a view is the journey's until it is left.** Opening a journey always shows the graph at the All detail, highlights off, conditional nodes shown, not-relevant hidden, fitted to the whole drawn graph; nothing of an earlier visit (pan, zoom, expanded containers, sort, the list's levels) is remembered. Switching between its views keeps the view controls' highlights, filter and search, which follow the viewer from view to view; each view otherwise opens with its own stock settings. Only what a control changes resets: the rows checked on a new query, the canvas's fit when the detail changes what it draws. Never remount a screen to refresh it. A proposal's review canvas is the one exception to the whole-graph fit on opening: it opens fitted to the nodes the proposal changes (with the whole graph when it changes none that are drawn), so a small edit to a large journey is readable, and the Fit button still fits the whole graph.
- **Graph cards** at reading zoom show the kind and status chip, the title, and one foot line. A body line appears only where it answers something at a glance, such as a decided decision's answer.

## Tokens

Use tokens everywhere. Screen code never uses raw hex values, pixel radii above `--radius-lg`, or its own durations.

### The neutral ramp and its roles

Every neutral comes from one cool ramp, `--n-0` to `--n-15`: OKLCH hue 250 throughout, lightness stepping from 0.985 to 0.14, chroma 0.004 at the palest rising to 0.032 at mid and settling at 0.012 at the darkest. A role is a step, never a hex of its own, and a theme only chooses which step each role takes. New neutrals are added by picking a step.

| Step | Hex | L | Step | Hex | L |
| --- | --- | --- | --- | --- | --- |
| `--n-0` | `#f8fafd` | .985 | `--n-8` | `#4b5a6a` | .46 |
| `--n-1` | `#f0f4f8` | .965 | `--n-9` | `#3a4047` | .37 |
| `--n-2` | `#e6ecf2` | .94 | `--n-10` | `#2d3238` | .315 |
| `--n-3` | `#dbe2e9` | .91 | `--n-11` | `#22272d` | .27 |
| `--n-4` | `#ced5dd` | .87 | `--n-12` | `#1b2025` | .24 |
| `--n-5` | `#b5bfc9` | .80 | `--n-13` | `#181c21` | .225 |
| `--n-6` | `#94a0ad` | .70 | `--n-14` | `#12171b` | .2 |
| `--n-7` | `#6d7c8c` | .58 | `--n-15` | `#060a0e` | .14 |

| Role | Token | Light | Dark |
| --- | --- | --- | --- |
| chrome (rail, inspector column, sheet, toast) | `--chrome-bg` | `--n-4` | `--n-14` |
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

### Derived

| Token | Light | Dark | Role |
| --- | --- | --- | --- |
| `--color-ground` | `--n-2` | `--n-13` | the plain ground behind workspace content, and under a canvas's ruling |
| `--color-sunken` | `--n-1` | `--n-12` | inside container frames, so the paper cards in them stand out |
| `--color-edge` | `--n-6` | `--n-8` | card, row and panel borders |
| `--color-frame` | `--n-7` | `--n-7` | container frames and menu borders |
| `--chrome-bg` | `--n-4` | `--n-14` | rail, inspector column, sheet, toast |
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

Dark mode follows the OS preference and can be forced with `data-theme="dark"` or `"light"` on the root element. Every theme-dependent token has a value in each theme (the tables above): the same ramp, with each role taking a darker step. The paper is raised (`--n-11`) above the ground and the chrome goes deeper than both, all of them muted (the dark end of the ramp is nearly neutral, so the dark pages read as soft graphite rather than navy), and the borders lighten so they keep their contrast. The accent and status colours are the same in both themes.

### Ramp palette

For ordered values, use `--ramp-1` to `--ramp-4`, four steps of the neutral ramp from dark to pale: `--n-12 --n-8 --n-6 --n-4` (dark: `--n-1 --n-6 --n-7 --n-9`). Show binary or ordinal attributes with shape, a ring or a label, not with more saturated colour.

### Shape, space, depth, motion

- Radii: `--radius-none: 0`, `--radius-sm: 0`, `--radius-md: 2px` (buttons, inputs), `--radius-lg: 4px` (largest allowed), `--radius-full: 9999px` (status dots only). Panels and cards are square.
- Spacing (8px base): `--space-1..7` = 4, 8, 12, 16, 24, 32, 48px. Use small steps inside a module and large ones between modules.
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

Mono uppercase is kept for labels, column headers, keys and status chips; buttons, menu items, toolbar text and tabs are sentence case (Approachable by default). The base size is 17px. Titles are sized for a tool, not a landing page. Fonts are self-hosted (`design/fonts/`, woff2, latin and latin-ext, SIL OFL 1.1) and nothing loads from a CDN at runtime. Fall back to `ui-sans-serif, system-ui` and `ui-monospace`.

## Layout

- **Shell.** Three zones, with nothing along the bottom:
  - a fixed rail on the left (brand with the sync dot at the right edge of its row, nav, small counts in mono) on `--chrome-bg`
  - a wide workspace in the centre on the plain `--color-ground`, with paper rows, cards and panels on it
  - an optional inspector on `--chrome-bg` for the detail of whatever is selected
  The chrome follows the theme: light chrome in light, dark in dark. The workspace runs edge to edge between the rail and the viewport. Only a canvas inside it is ruled.
- **One scroller per region.** At 720px and wider the frame fills the viewport (`100dvh`) and the document never scrolls. The rail, the workspace body, the inspector body, a sheet body, a popover body and a modal body are the only scrolling elements, each scrolling once. A gesture surface (a graph or timeline canvas, which owns the wheel and touch) never sits inside a scroller. A primary table is its region's scroller, so its sticky header sticks to the right box; secondary tables (inside the inspector or a card) never scroll and end in "Show all" instead.
- **Inspector.** A right-hand column, or a resizable sheet along the bottom when the workspace must keep its full width. Both are chrome, with paper cards inside. One control hides it: the head's "Hide inspector" button (a `›|` chevron, tooltip "Hide the inspector  ]"), with no close button, since deselecting is the canvas background click or Esc. Hidden, the column folds to a 32px rail on the right edge, still visible, with a `|‹` "Show inspector" button and vertical mono INSPECTOR and ASSISTANT tabs (the column's own tabs turned on their side; Assistant only where it is offered): a tab opens the column on that tab, and `]` toggles it. Selecting a node (a canvas click or a list row) brings a hidden inspector back. The sheet has the same single control, and hidden it is a slim handle bar along the bottom that opens it.
- **Grid.** Inside the workspace, modules sit on repeating `minmax(0, 1fr)` columns.
- **Responsive.** From 720 to 1099px the inspector becomes a bottom sheet over the workspace (a drag handle, two snap heights, Esc closes it) rather than dropping below the workspace, so the document still never scrolls. Below 720px everything stacks into one scrolling column and the rail becomes a top bar with wrapping nav; a gesture surface there is an inert preview (a fitted, static picture a swipe scrolls past) with one button that opens it full screen, where gestures belong to it. Supported from about 390px to 2560px.
- **Placement.** Place a reused component from its container (its flex, grid and outer margins), so it can sit in a new container without carrying an old one's layout.

## Components

- **Panel / card.** Opaque paper surface, four corner ticks (L-shaped 1px strokes, 8px long, inset 3px), a 1px border, `--shadow-sm`, square corners. No ruling: only a canvas is ruled (see Charts and graphs). A panel's first label becomes its header band: `--color-band`, a bottom hairline, and an uppercase mono label in steel. Cards have a header band and an optional action row with a top hairline. Never give one edge a colour. Inspector sections are the exception: each is a sentence-case Plex Sans heading row, folded by default, showing its name and at most a count.
- **Label.** Uppercase Plex Mono 12px, letter-spacing .08em, steel. Use it for module titles, field labels above controls, and keys in key/value tables.
- **Table.** The one row component for every list of nodes (the Plan list, the frontier, folds, Mine, decision lists): opaque paper rows with a `--color-edge` hairline under each, the table inside a bordered panel on the ground, a status chip column and an owner column. Rows have room (a two-line row about 64px, a one-line row 8px 12px padding); vertical hairlines between columns only in data tables opened in full. The header is a sticky band with steel labels, uppercase mono in a data table and sentence case in the Plan list, where a column that sorts is a button. Numbers are right-aligned mono with tabular figures, and secondary parts of a cell (a percentage, a rank) are steel. Hover is a neutral tint. A tree is a classic tree: for each ancestor with more children below, a continuous 1px `--color-border` vertical line through the full row height (across the row hairlines) centred on that ancestor's chevron, and for the row itself a `├` or, as the last child, a `└` joining the line to its chevron or bullet. A container's fold is a real 24px icon button with a 12px chevron in `--color-text`; a leaf has a small bullet in the same slot, so every title starts at its level's 24px column. A container row (a group or a stage) is a `--color-band` row with a 600-weight title and its progress, so it reads as the header of the rows under it. A tree keeps the levels a person opened or folded when the row set changes (a filter, Show not relevant). A primary table is its region's scroller, so its sticky header sticks to the right box; secondary tables (inside the inspector or a card) never scroll and end in "Show all" instead.
- **Selection.** A selected row, card or list item gets a 2px accent line on its left edge (an inset box-shadow) and the faint blue tint. A selected container (a frame) is the 2px accent frame and the faint tint over its whole body, head included; only the selected node is tinted: the cards and containers inside it keep their trace marks (NEEDS and UNBLOCKS) like any other node, and so do its ancestors and the nodes outside it. The keyboard focus ring is a 2px accent outline on every interactive element. Selection and focus are the main uses of the accent.
- **Status.** An uppercase mono label in a hairline chip, with a 6px dot: success (done or approved), error (failed or rejected), warning (deferred or attention), steel (pending), hollow (superseded), and ink, filled for actionable now and a ring for in progress. The text is always there; the dot only backs it up. No two states share a dot shape: the blocked dot is a 6px square and a skipped dot is hollow and struck through, so it differs from a hollow superseded or not-relevant one. Done, decided and reached draw a check in the dot's place: a mask in the success colour, as tall as the chip's capitals and stroked as heavy as its text, so it scales with the chip. A done card's chip is hollow, in the success colour. Where the word cannot fit (a narrow timeline row), a hover gives it.
- **One status language: fill says state, opacity says focus, and the lowest opacity wins.** A canvas card or container, and a list or tree row, is 1 when it is open and in focus, 0.5 when it is done (done, decided, reached), and 0.3 when it is out of focus (outside a trace, or not lit by Frontier or Decisions highlighting), whatever its state; the levels are never multiplied, so a done card out of focus is 0.3. Done work keeps the neutral fill with muted text and the green done chip; it is faded, not filled. A done row fades its content cells (not the checkbox column, and not the row's fill, so hover, selection and focus stay full strength). A container fades its own head and frame only: its children are separate cards, so open work inside a done container stays at 1, and a container holding a lit node is not faded. Skipped and not relevant are hollow (no fill), not relevant also muted; open work is paper. A ruled-out row is muted. No state takes space from titles at zoom-out.
- **Buttons.** Hairline box, paper fill, Plex Sans 14px, `--radius-md`. Hover darkens the border to steel. A primary button is filled with graphite ink (light rail ink in dark mode), never blue. Ghost buttons are transparent. Disabled buttons are at 45% opacity.
- **Inputs and selects.** A hairline box at `--control-h`, body type, and a hairline that turns steel on hover. On focus the border turns to the accent. A select is square with a drawn steel chevron, and every select in the product shares that one style. Checkboxes, radios and ranges take `accent-color: var(--color-text)`, not blue. Number, date and textarea inputs use mono.
- **Links.** Ordinary links are the text colour with a 1px underline in `--color-secondary` that turns to `currentColor` on hover; they are never blue (blue is selection, focus and downstream).
- **Keyboard hints.** A `kbd` chip: chrome ink on chrome with a hairline border. `?` opens a modal key sheet (a key-value table of `kbd` chips, the canvas's keys under a Graph heading) for the current screen.
- **Toast.** A chrome chip at the bottom right of the workspace, 12px above the window's bottom edge (above a sheet when one is open). Mono 13px, with `--shadow-md`. It rises in over 180ms, leaves after 2.5s, pauses on hover, has no close button, and a new one replaces the old. Saves never raise a toast; the sync chip reports them.
- **Sync dot.** There is no status strip; the sync state is one small button at the right edge of the brand row, at the top of the rail (and of the phone's top bar): a single 10px mark, never a text label or a second line. Its sentence is its accessible name. Hovering or focusing it shows the popover (revision, what needs you, recent saves), which goes when the pointer or focus leaves; clicking pins it open until Esc or a press outside. The popover is anchored just below the dot, fixed so the rail does not clip it. Its colour says what it asks of you: green, nothing; steel, working on it; amber, safe to wait (it will sort itself out); red, your change is not landing until you act. Each kind of state (the tone) also has its own shape (disc, ring, diamond, square, a "!" for a conflict, a dotted ring for sample data), so colour is never the only signal.
- **Segmented control.** A hairline row of segments at `--control-h`, sentence case. The active segment gets the band fill and a 2px ink underline (the active-nav marker), not the accent: it shows where you are, not a selection. The projection switcher is one, always at the right end of its toolbar.
- **Popover and menu.** A paper panel with a 1px `--color-frame` border and `--shadow-md`, anchored to its control (a context menu opens at the pointer), its body the one scroller (capped at `min(60dvh, 480px)`), closed by Esc. Menu items are sentence case, 14px, padded 4px 12px, with a neutral hover and an optional right-aligned mono key hint. Items are grouped by how often they are used, with a hairline between groups and an optional mono group label above one.
- **Option list.** A choice among a few answers (a decision's choices, a radio group): a bordered stack with a hairline between options. The chosen option is banded (`--color-band`) with an inset 1px ink ring, and what picking an option does is right-aligned in mono steel.
- **Group frame.** A container drawn around its members (its children sit one frame padding, 20px, in from the head rule, the bottom edge and both sides alike, so their NEEDS and UNBLOCKS tags never touch the rule): a 1px `--color-frame` border, a translucent paper body (no sunken fill, no filled header), and a head of three lines: the kind label (with CURRENT), the status chip and the fold control; the title; the progress meter with its "0 of 8 done" and any warning words ("needs breakdown") right-aligned on the same line. Under the head a rule across the frame's full width, in the frame's own stroke (its colour, and its thickness when selected), ends it; the "Waits on N hidden prerequisites" marker sits just below the rule. A container shows no owner; a date, when it has one, is a fourth line in the head; where a node came from is told in the inspector's Origin section only, never on the canvas. Below the near zoom the head is simplified to its title, left aligned and centred in the head, with a stage's state chip (not a group's) at the far right: the kind, progress, date, owner, marker and fold control are not drawn, a double-click still folds, and the rule stays.
- **Tooltip.** Every control carries a native `title`: what it does, plus its key where it has one, and a badge on it says what it counts in the same title. The one exception is the sync dot, whose status popover opens on hover and focus and says it instead. There are no first-run hints or custom tooltips.
- **Bottom sheet.** A chrome sheet with a 24px drag handle (a short hairline bar), snapping to two heights; its body is the one scroller.
- **Multi-select.** Selection is one set: one member is inspected and traced; several are listed and acted on in the inspector. There is no select mode and no selection bar. On the graph a plain click on a card (title or body) selects it alone; Shift-, Cmd- or Ctrl-click adds or removes it; a Shift-drag on the background adds the cards in the box; a background click or Esc clears; on touch the card menu's "Add to selection" does the same, and Shift+Space on a focused card toggles it. With one member the inspector shows the node and the graph its trace. With two or more it shows "N selected" and Clear, the members (title, kind, status, and a × to drop one), then the actions in a fixed order (Status, Assign owner, Snooze); the graph tints the members and draws no trace, and a folded inspector unfolds. An action every member's state does not allow stays visible and disabled, its reason in its tooltip. Changing the members closes an open form. The list's row checkboxes (and Shift- or Cmd-click on a row) add to and remove from the same set, so the inspector shows the same panel there; a plain row click opens that one node, and the set survives a switch between the graph and the list.
- **Rank tag.** A small square hanging off a card's top-left corner: the primary-button pair (`--ink-fill` with `--color-surface` mono figures), so it flips with the theme.
- **Hanging tag.** A small mono label hanging off a card's top-right or bottom-right edge for transient or lens information (a trace role, a signal value). Each slot has one owner, so two tags never displace each other.
- **Dates.** Words first: `Decide by Oct 14`, `Due in 3 days`, `3 days late`. A date is `Oct 14` in the current year and `Oct 14, 2027` otherwise; ISO dates appear only in tooltips and History. A soon date is in warning ink and an overdue one in error ink, always with its words. Dates in a column or on an axis are mono with tabular figures.
- **Diff and log blocks.** Mono, hairline border. Added and removed lines get `--color-add` and `--color-del`, and hunk headers are steel.

## Charts and graphs

- **The ruling lives on canvases.** The 24px ruling (`--ruling`) on `--color-ground`, with corner ticks, is drawn by the graph canvas, the timeline and the phone map preview, where it reads as a measuring surface. Content never sits on it anywhere else.
- A faint scaffold: gridlines in `--chart-grid` (crisp 1px) and a steel axis. Axis text is mono 11px in steel.
- Marks are graphite (`--chart-ink`), with a second series in steel (`--chart-steel`). Lines are 1.5px.
- Graph cards have a `--color-edge` border and keep their ticks. A card is always drawn whole and only scales with the zoom: there is no title-only level, and at fit the text is too small to read, by design. A container's head, below the near zoom, counter-scales its title continuously: 13px on screen, never below the reading size unless its room needs it, and never more than the largest size at which the whole title fits the head. It breaks only between words, never inside one, and shrinks (a narrow nested head may go below 13px on screen) before it ends in an ellipsis on a whole line. A stage's chip beside it is set at about 10.5px on screen and is dropped when it would make the title smaller or wrap to more lines.
- **Selecting on the graph.** Clicking a card selects it and traces it: the trace colours what it needs and what it unblocks and tags those cards NEEDS and UNBLOCKS, and there is no bar of counts. Clicking the empty canvas, or Esc, deselects it: the route goes back to the view without the node, the trace clears, and the inspector shows the journey. A drag that pans the canvas is not a click and never deselects.
- **The Signal button.** One button at the canvas's top right, `Signal: Off` or `Signal: Gravity`, pressed (band fill, ink underline) when a signal is on, with the tooltip "Which priority signal each card shows: rank, gravity, unlocks or slack". Its menu is the Off, Rank, Gravity, Unlocks and Slack options, a hairline, then "Key to the lines". There is no separate chip for the active signal.
- **Controls floating on the canvas** (the view controls, the Signal button, the zoom buttons and Reveal) sit on `--color-surface` with a 1px `--color-edge` border and `--shadow-sm`, so they read against the ruling in both themes, and take the pointer only on their own boxes, so a drag or a Shift-drag between them still reaches the canvas.
- **Fits clear the controls.** The view controls run along the canvas's top left (on a phone one View button, with the detail and filter badges, holding the same controls in its menu) and the Signal button sits at its top right; the zoom buttons are a vertical stack at the bottom left (the keys hint `?` is not here: it is a glyph on the rail's You line, so every screen has it), and Reveal sits at the bottom right. A fit pads for those zones (56px top, 72px left, 24px right, 56px bottom, more under a sheet), and a node picked under a sheet is centred in the room left inside them. A fit always shows the whole drawn graph, however small it makes the cards.
- **The view controls.** One row of Detail (All, Deliverables, Decisions, Stages, general to specific, a menu), Frontier, Decisions, Filter and Search, the same on the graph (floating at the canvas's top left), the list and the timeline (at the top of the pane). Detail and Filter choose which cards are drawn; Frontier and Decisions only highlight cards already there and never change detail, folds or camera. A control never grows text to say its state: Filter wears a small count badge when it hides something ("3 not relevant hidden") and Detail when a highlight matches cards its level cannot draw ("2 hidden at this detail; choose All to see them"), the words in the tooltip.
- **Reveal.** A small floating box at the canvas's bottom right, present only while the selected node's trace has nodes without a card of their own: "Reveal 5 linked nodes", then "Hide revealed nodes". There is no status bar; nothing else is said along the bottom.
- A selected or anomalous mark is the accent, and a threshold or reference value is a dashed accent line (1.5px, dash 4 3).
- Heat scales run from steel to blue. A screen that needs categorical colour adds a palette with no hue near the accent.
- In graphs, edges are 1px hairlines: steel, whether or not what they join is done: a line between done cards gets no special quieting. An edge's kind is told by its line style (solid or dotted) and its end marker (an arrowhead, a hollow diamond, a bar), never by a text label on the line; a hover gives its sentence. Lines keep their width at every zoom.
- Selecting a node traces it: everything it needs is drawn in ink, and everything it unblocks in the accent, as clearly emphasised lines whatever their dash, finished or not; a requirement into a container counts when the container holds something the selection needs (and out of one, something it unblocks). Each traced node carries a NEEDS or UNBLOCKS tag so colour is never the only signal, and everything else goes out of focus (the status language above), lines at one fixed subordinate level. A line whose end is hidden (by the detail step, a folded container or a roll-up) is drawn to the card that stands for that end; one that would join a card to itself or to a card around it is not drawn, lines between the same two cards are one, and a line never ends where no card is. Resting lines stay readable against the ruling in both themes, never washed out. Hovering an edge draws it and its two ends in ink.
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
- **Density.** The library is dense throughout. This version shows less by default and roomier rows (Approachable by default), keeps the ruling (on canvases), ticks and borders, and adds the sync dot, segmented control, popover, bottom sheet, selection bar, and rank and hanging tags.
- **Chrome and ruling.** Chrome follows the theme, and the ruling is only on canvases, so content never sits on a grid.
