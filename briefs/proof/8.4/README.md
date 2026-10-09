# Proof for brief 8.4: The app frame and the Graticule migration

The app now sits in one frame: a graphite rail, the paper workspace, an inspector column, and a
28px strip, all in Graticule's tokens and fonts, light and dark. Each region scrolls on its own, so
a node's History is always reachable and the page never scrolls behind it. A phone gets a map
preview that leaves swipes to the page, and a full-screen map that owns them.

## Before and after

The canvas with a node open. Before, the detail was a sticky box inside the page; now it is the
inspector column, and the canvas fills the workspace:

![Before](before-canvas-light.png)
![After](canvas-light.png)

The list. Before, a system-font table in a page column; now a primary table in the workspace, its
band header sticking to the region:

![Before](before-list-light.png)
![After](list-light.png)

The inspector scrolled to the end. Before, History sat on the window's edge with the page scrolled
behind it; now History is open, inside the window, and the document has not scrolled:

![Before](before-inspector-history.png)
![After](inspector-history.png)

The decision view follows the same rule: one view per page. The graph fills the workspace, and the
table of what each answer affects is the other half of a Graph | Table toggle, so the wheel over the
canvas never lands in a scroller:

![Decisions](decisions-graph.png)

## Dark

The rail, inspector and strip sit on the ground colour, so the paper does not merge with its chrome.
The You page has a System, Light and Dark control that is kept in this browser:

![Canvas in dark](canvas-dark.png)
![List in dark](list-dark.png)
![Theme control](you-dark.png)

## Tablet and phone

From 720 to 1099px the inspector is a bottom sheet (40dvh or 85dvh) whose grip, tabs and close sit in
one head row. The workspace ends above it, and the picked card is centred in the band it leaves:

![Sheet](tablet-sheet.png)

Below 720px the map is an inert preview with one button; Open map is a full-screen layer (`map=1`,
so Back closes it) where a tapped node opens as a sheet over the map:

![Preview](phone-page.png)
![Full-screen map](phone-map.png)
![Node sheet](phone-sheet.png)

Known limits:
- The journey head is still several rows (the journey pages brief shrinks it); a taller-than-window head scrolls the region.
- The rail has no Recent list yet, and the strip still carries the old Live badge until the sync chip lands.
