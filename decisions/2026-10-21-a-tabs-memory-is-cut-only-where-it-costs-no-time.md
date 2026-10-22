# A tab's memory is cut only where it costs no time

Question (from cutting a tab's fixed memory cost): four cuts were proposed: start the layout worker on the first layout and stop it when no canvas is shown; build the wasm module for size; drop the page's second engine instance on the server host; and open one tick stream per tab instead of one per view. Each was to be kept only if it measurably saves memory and costs no time a person feels. Which land?

Call: the second engine instance goes (its own record: decisions/2026-10-21-the-page-asks-the-derive-worker-for-every-engine-call.md), and the wasm module is built for size: `opt-level = "s"` in the `wasm` profile, then `wasm-opt -Oz` (pinned in `mise.toml`) over the bound module. The layout worker on demand and one stream per tab stay out. The rule for the size build, from the owner: take it when it saves memory and the mean derive cost at normal sizes (the bundled journeys, and documents up to half the node limit) is under 10 ms. The rule is met by `s` with `-Oz`, which also has the best memory saved for the time lost of the size builds.

Measured on a local `cairn demo` in headless Chromium, the tab's private footprint (MB, median of five tours run in turn; a tour opens four journeys' views and nodes and three routes):

| Build | Home | First graph | After a tour | After a second | Index, idle | Peak |
|---|---|---|---|---|---|---|
| main | 118 | 135 | 163 | 141 | 140 | 163 |
| layout worker on demand | 102 | 130 | 141 | 120 | 121 | 143 |
| no second engine | 116 | 129 | 158 | 143 | 140 | 158 |
| one stream per tab | 116 | 136 | 161 | 140 | 138 | 161 |
| all three | 98 | 133 | 134 | 118 | 116 | 136 |

- The layout worker on demand saves 16 to 20 MB but makes every graph opened while it is stopped wait for ELK to start: over fifteen runs each, a tab's first graph took about 45 ms longer (median 446 to 471 ms against 403 to 417), and a graph opened after the journey index about 90 ms longer (122 to 133 ms against 34 to 39). Starting it when a canvas mounts, rather than on the first layout, hid none of that. It stays started at boot.
- One stream per tab cuts the streams a tour opens from 69 to 1, but saves no memory that can be told from noise, and it needs the stream to say when its first, complete take ends before a view that starts watching later can learn that its journey was deleted while the stream was down. It is not worth that for nothing measured.
- The size build, measured against `opt-level` 3 with the second engine already gone (the module in the derive worker's own script, a fresh worker and an empty cache each time; 40 rounds of every build run in turn under `nice`, then a tab tour as above, ten tours of the two builds compared):

  | | level 3 (before) | `s` with `-Oz` (kept) |
  |---|---|---|
  | derive, the four bundled journeys (8 to 27 nodes) | 0.5 to 1.7 ms | 0.3 to 1.1 ms |
  | derive, 500 nodes (25% of the node limit) | 19.2 ms | 19.5 ms (+0.3) |
  | derive, 1,000 nodes (50%) | 34.7 ms | 38.8 ms (+4.1, +12%) |
  | derive, 2,000 nodes (100%) | 80.2 ms | 86.4 ms (+6.2, +8%) |
  | derive, every limit at once | 588 ms | 648 ms (+60, +10%) |
  | cold first derive: compile, start the worker, first journey (median, local server) | 47 ms | 35 ms |
  | module, as built | 8.84 MB | 5.72 MB |
  | module, gzip / brotli | 2.52 / 1.54 MB | 1.80 / 1.16 MB |
  | tab: home, after a tour, idle | 114, 159, 138 MB | 102, 144, 127 MB |

  The memory saved is 11 to 15 MB at every checkpoint. The mean cost over the bundled journeys, 500 and 1,000 nodes is 0.5 ms, and no size in that range costs more than 4.1 ms; it only passes 6 ms at the node limit, which no journey or route comes near. The module also compiles faster (17 against 38 ms), so the first derive on a cold page is 12 ms sooner before any download time, and the download is a quarter smaller. The other builds, over the same runs: `s` alone costs +7.9 ms at 1,000 nodes (+23%) and saves 3 to 10 MB; `z` alone derives 77 percent slower at 1,000 nodes; `z` with `-Oz` saves up to 4 MB more than `s` with `-Oz` but costs +9.0 ms at 1,000 nodes and +164 ms at every limit; `-Oz` over level 3 saves 2 to 5 MB, within the noise of the tours, and no derive time. Compression is the server's or the edge's, not the binary's (it serves the module as is); `-Oz` makes the module smaller as built but compresses worse than `s` alone (1.16 against 0.99 MB brotli), which is the price of the memory it saves.

Alternatives: keep the layout worker on demand and accept the wait (the largest saving, about 20 MB); start it on the first canvas and never stop it (saves memory only in a tab that never opens a graph, still about 45 ms on the first one); `s` without `-Oz` (no extra tool, a smaller download once compressed, but 3 to 10 MB less saved and 4 ms slower at 1,000 nodes); `z` with `-Oz` (the most memory, but a derive 16 to 28 percent slower from 500 nodes up).

What would change it: a derive budget that counts the time at half the node limit (4 ms there today), or a journey index that derives many large journeys at once, would bring back level 3 for the module; memory becoming the tighter budget (many tabs on small devices) would bring back the layout worker on demand, then `z`. A stream cap or a server that charges per connection would bring back one stream per tab. `wasm-opt` failing to read a module that a newer compiler emits with more features would be fixed by naming the features in the build task, or by dropping the pass (level `s` alone keeps the derive time and part of the memory).
