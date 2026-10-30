# Graticule design language

Graticule is the design language for dense, keyboard-driven tools read closely. It looks like a measurement workspace: opaque cool-white paper rows, cards and panels on a plain ground, each bordered and pinned by four corner ticks, with a faint 24px ruling only on the canvases people gesture on (graph, timeline). Light and dark are both native, chrome included. Type, borders and opaque surfaces carry hierarchy; colour is kept for state.

Token values live in `design/tokens.css`, base styles and shared components in `design/base.css`, fonts in `design/fonts/`; `design/example.html` shows the components. On a rule, this document wins over them. Screens are judged by their screenshots against these rules; this document holds the shared language, not each screen's layout.

## Principles

- **Structure before polish.** A screen holds what its job needs and no more: remove or merge before restyling.
- **One concept, one form.** A thing looks and behaves the same everywhere; sibling views of the same data share controls and meaning. A reused component is placed by its new container, never carrying an old one's layout.
- **Structure over colour.** Hierarchy comes from type (grotesk titles, mono labels, tabular numbers) and hairlines. A screen that needs colour to be readable needs better structure.
- **Keep structure visible.** Every item is an opaque surface with a border visible against its ground in both themes.
- **Accent means state.** The electric blue marks selected, focused or flagged, rarely. Never decoration, a wash, a category, or ordinary links and buttons.
- **Measured and dependable.** Square corners, crisp 1px lines, mono metadata, tabular figures.
- **The settled state comes first.** Every screen reads correctly with no motion; motion only adds.

Avoid: rounded cards, pastel fills, gradients, illustrations, decorative motion; rows of floating cards with coloured edges; removing borders or corner ticks; ruling anything but a canvas; chrome that ignores the theme; status by colour alone.

## Approachable by default

Every screen is calm at first sight and denser only on request. This section wins over any denser default elsewhere.

- **Show less by default.** A row or card shows its title, its status and at most one secondary fact; the rest is a hover, click or fold away. A row's primary action and owner stay on the row: only facts go behind a hover, never a control. Leave a count where something is hidden.
- **More air.** Separate with space before hairlines: small steps inside a module, large ones between modules.
- **Fewer shouting labels.** Uppercase only where Typography says; at most one chip per row or card.
- **Plain words over codes.** "Due in 3 days", "3 days late", "Unlocks 2"; never a bare number or abbreviation. An empty section is absent, a missing value is said in words, never a dash, and a screen never claims what Cairn does not know (no "All clear").
- **Quiet chrome.** A header is one line; secondary actions live under one `⋯` menu; no status strip.
- **Detail on demand.** The inspector leads with one plain sentence and the primary action; below, folded rows show a name and a count, never a key-value summary. History is sentences naming people ("Ana snoozed it until Oct 31").
- **A control that cannot act is absent**, unless the person needs to learn why it is held: then it is shown held, with its reason.
- **Stock views.** A journey opens at its stock view and remembers nothing from an earlier visit. Switching views keeps what is drawn (Depth, Frontier, Filter, search) and resets only each view's own settings. Never remount a screen to refresh it.

## Tokens

Screen code uses tokens only: no raw hex, no radius above `--radius-lg`, no own durations or sizes. A theme-dependent token is one `light-dark(light, dark)` declaration.

- **Dark theme.** Follows the OS, forced by `data-theme` on the root. The same ramp in darker steps, borders lightened to keep contrast.
- **Neutrals** are steps of one cool ramp. A role takes a step, never a hex of its own.
- **Status colours** are set as text. The brand colour is quiet identity, never state.
- **Contrast floors** in both themes are enforced by `web/app/src/ui/tokens.test.ts`.
- **Ordered values** use the ramp; binary or ordinal attributes use shape, a ring or a label, not saturation.
- **Shape.** Panels and cards are square; buttons and inputs slightly rounded; only status dots are round.
- **Borders** are 1px; 2px only for marks of selection and the current place (accent line, active nav marker, active segment, current menu item) and the proposal layer's change edge.
- **Touch targets.** On a coarse pointer every tappable control has a hit area of at least `--touch-target`, applied once to the shared controls. Grow the hit area, not the drawing. Links in running text and everything drawn inside the canvas are exempt: zooming is how a card grows.
- **Motion** is small additions only (a drawer slides, a highlight fades, a toast rises), off under reduced motion.

## Typography

| Role | Face | Notes |
| --- | --- | --- |
| Titles | Space Grotesk 700 | page and section titles uppercase; inspector headings and a node's title (as authored) sentence case |
| Labels: module label, column header, key, status chip | IBM Plex Mono 12px / 500 | uppercase, steel |
| Prose | IBM Plex Sans 17px | |
| Rows, body, controls, buttons, menus, tabs, helper text | IBM Plex Sans 15px | sentence case |
| Numbers, ids, paths, emails, code | IBM Plex Mono | tabular figures; keep their case |

12px is the floor: no drawn text is smaller. Fonts are self-hosted.

## Layout

- **Shell.** A rail on the left, the workspace edge to edge, and an optional inspector for the selection. Rail and inspector are chrome.
- **One scroller per region.** From 720px the document never scrolls; each region's body scrolls once. A gesture surface (graph or timeline) never sits inside a scroller.
- **Responsive**, from about 390px to 2560px. Below 1100px the inspector is a bottom sheet; below 720px everything else stacks into one column. A phone drops no fact a desktop shows: rows stack instead.
- **Grid.** Modules sit on repeating `minmax(0, 1fr)` columns.

## Components

- **Panel / card.** Opaque paper, four corner ticks, 1px border, small shadow, square, never ruled. A panel's first label becomes its header band. Never colour one edge.
- **Label.** The 12px mono uppercase steel label: module titles, field labels above controls, keys.
- **Table.** The one row component for every list of nodes: paper rows with a hairline under each, a status chip column, sticky header. A tree shows its levels with continuous guide lines; a container row is banded with its progress. A primary table is its region's scroller; a secondary one never scrolls and ends in "Show all".
- **Selection.** A 2px accent line on the left edge plus the tint; a selected container, a 2px accent frame. Focus is a 2px accent outline on every interactive element.
- **Status.** A mono uppercase word in a hairline chip with a dot. The word is always there, and no two states share a dot shape (blocked is a square; skipped is hollow and struck). Done and decided show a check.
- **One status language: fill says state, opacity says focus, and the lowest opacity wins.**
  - Levels: 1 in focus; 0.7 snoozed; 0.5 done or decided; 0.3 out of focus (outside a trace, unlit by Frontier, faded by depth). Never multiplied.
  - Blocked and scheduled work is never dimmed.
  - Fading touches content, never selection, hover, focus, or controls.
  - Lists read for finished work keep done rows at full opacity.
- **Buttons.** Hairline box on paper; primary is ink-filled, never blue. Disabled is 45% opacity and keeps its value.
- **Inputs and selects.** Hairline box at `--control-h`; accent on focus. Fields carry a label above, never a placeholder alone. On a phone a text field is 16px, so the browser never zooms into it.
- **Multi-value input.** One growing box of chips then the input. A value that fails stays in the input with the reason under it. This is the one chip: an active filter is the same chip.
- **Links.** Text colour with an underline; never blue.
- **Segmented control.** The active segment has the band fill and an ink underline, not the accent; so does a pressed toggle.
- **Popover and menu.** Paper, framed, anchored to its control; its body scrolls; Esc closes and returns focus. Items are sentence case, grouped by frequency.
- **Modal.** A native `<dialog>`, its body the scroller, Esc closes. Used rarely.
- **Toast.** A chrome chip at the bottom right that leaves on its own; a new one replaces it. Saves never toast: the sync dot reports them.
- **Sync dot.** One mark whose colour and shape say what it asks of you: nothing, working, safe to wait, or act or your change will not land. Its popover says the rest.
- **Tooltip.** One component, never the browser's `title`. A tip says only what the screen does not: what pressing does, its key, what a count counts, why a held control cannot act, cut text whole. Never a tip that repeats adjacent words. A fact whose only place is a tip is a toggletip, reachable by keyboard and tap.
- **Guidance** appears when needed, is dismissible, and never returns once seen; the tour is the guided first run.
- **Multi-select.** Selection is one set shared by the graph and the list; one member is inspected and traced, several are listed and acted on together in the inspector. No select mode.
- **Deleting for good** asks once, naming what goes and how many; no typed name.
- **Proposal layer.** Reviewing a proposal is the destination's own page with a layer on, never a separate screen. Changes are marked by fill, a styled edge (solid added, dotted changed, dashed removed), and a leading word, so colour is never the only cue. Under the layer the page offers no live write.
- **Rank tag and hanging tag.** Small labels off a card's corners for rank and transient lens information; each slot has one owner.
- **Nothing spills out of its box.** Every box holds its content; text is never cut without an ellipsis or clamp mark. Only tags, badges, handles, popovers, menus and tooltips overhang, each marked `data-overhang`. Text yields before controls: controls and chips never shrink. Browser tests enforce it on every screen, and the stress journey holds the worst content the limits allow.
- **Names, not keys.** No screen, tooltip or error prints an internal key, digest, or revision number where a name belongs. What is gone is said in words ("someone no longer here"). Browser tests enforce it.
- **Dates.** Words first: `Decide by Oct 14`, `Due in 3 days`, `3 days late`. The year only when it is not this year; ISO only in tooltips. Source words: `pinned`, `recorded`, `worked out`. Soon in warning ink, overdue in error ink, always with words. A time of day follows its date in 24-hour mono, only where the date has one.

## Charts and graphs

- **The ruling lives on canvases**: only the graph and the timeline.
- Marks are ink, a second series steel; a selected or anomalous mark is accent. An ordinal signal on nodes is a labelled chip, never border weight or opacity. A relative axis labels days from the start ("Day 7"), never a calendar date.
- **Graph cards** show kind, status, title, and one foot line with the date and, only when it is the viewer or missing, the owner. A body line only for a decided answer, a container's progress, or a leaf's checklist.
- **The graph zoomed out shows its structure, not its cards.** Below reading zoom a container is a tile showing its title, state, and progress, dropping detail as it shrinks. Zoom alone decides; nothing moves.
- **Group frame.** A group draws as a frame around its members, with a head showing its title, state and progress.
- **Edges** are 1px steel at every zoom; kind is line style and end marker, never a label. A line to a hidden node is drawn to what stands for it.
- **Tracing.** Selecting a node draws what it needs in ink and what it unblocks in accent; everything else goes out of focus.
- **Date lines** are not edges: dashed, drawn only for the selected node while its dates are open.
- **The view controls** are one row on every drawn view (Depth, Frontier, Filter, Search); the Summary has none, and a route has no Frontier. What a view cannot honour stays set, disabled with its reason. A count badge says what the settings hide. When nothing is drawn, the view says so and offers to reset.
- **Fits clear the controls** and always show the whole drawn graph.
- A pan is never a click.

## Accessibility

- Text meets WCAG AA, 4.5:1.
- Content shown on hover or focus is dismissable with Esc, hoverable, and persists until dismissed.
