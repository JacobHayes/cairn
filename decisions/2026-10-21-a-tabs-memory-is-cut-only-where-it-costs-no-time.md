# A tab's memory is cut only where it costs no time

Question (from cutting a tab's fixed memory cost): four cuts were proposed: start the layout worker on the first layout and stop it when no canvas is shown; build the wasm module for size; drop the page's second engine instance on the server host; and open one tick stream per tab instead of one per view. Each was to be kept only if it measurably saves memory and costs no time. Which land?

Call: only the second engine instance goes (its own record: decisions/2026-10-21-the-page-asks-the-derive-worker-for-every-engine-call.md). Measured on a local `cairn demo` in headless Chromium, the tab's private footprint (MB, median of five tours run in turn; a tour opens four journeys' views and nodes and three routes):

| Build | Home | First graph | After a tour | After a second | Index, idle | Peak |
|---|---|---|---|---|---|---|
| main | 118 | 135 | 163 | 141 | 140 | 163 |
| layout worker on demand | 102 | 130 | 141 | 120 | 121 | 143 |
| no second engine | 116 | 129 | 158 | 143 | 140 | 158 |
| one stream per tab | 116 | 136 | 161 | 140 | 138 | 161 |
| all three | 98 | 133 | 134 | 118 | 116 | 136 |

- The layout worker on demand saves 16 to 20 MB but makes every graph opened while it is stopped wait for ELK to start: over fifteen runs each, a tab's first graph took about 45 ms longer (median 446 to 471 ms against 403 to 417), and a graph opened after the journey index about 90 ms longer (122 to 133 ms against 34 to 39). Starting it when a canvas mounts, rather than on the first layout, hid none of that. It stays started at boot.
- Building the module for size (`opt-level` "s" or "z", with or without `wasm-opt -Oz`) saves 9 to 13 MB but derives 10 to 18 percent slower at the node limit (a 2,000-node document 88 to 101 ms against 76; every limit at once 594 to 634 ms against 538); `-Oz` alone over level 3 saves nothing measurable. The module stays at level 3 with no extra pass.
- One stream per tab cuts the streams a tour opens from 69 to 1, but saves no memory that can be told from noise, and it needs the stream to say when its first, complete take ends before a view that starts watching later can learn that its journey was deleted while the stream was down. It is not worth that for nothing measured.

Alternatives: keep the layout worker on demand and accept the wait (the largest saving, about 20 MB); start it on the first canvas and never stop it (saves memory only in a tab that never opens a graph, still about 45 ms on the first one); build for size at `"s"` with `-Oz` (best memory per derive time lost).

What would change it: memory becoming the tighter budget (many tabs on small devices), or a graph's first open no longer being on a path someone waits for, would bring back the layout worker on demand first, then the size build. A stream cap or a server that charges per connection would bring back one stream per tab.
