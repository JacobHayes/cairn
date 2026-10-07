# Proof for brief 4.5: Wasm host

The engine and the service now run in the browser. A journey's document from the server is
derived in a web worker by the engine compiled to wasm, and the in-browser root runs the
whole service over the memory store, seeded with the fixtures, with no server at all.

## What it does

- Every call over every fixture answers in Chromium exactly what the server answers, byte
  for byte: the derive, every canvas level, each node's trace and explanations, the decision
  view, timeline, status summary, next list, list, mine, snapshot, proposal previews, a local
  apply and its touched set, and route files exported and imported back. The same holds after
  a patch, after an entity merge, and for the document at the limits (2,000 nodes,
  containment at the depth limit).
- The in-browser root loads every fixture journey, applies a note on the vendor evaluation,
  and refuses two stale patches from the same base: the one writing the same note surfaces
  as overlapping, the one writing its own note is safe to resubmit and lands at the next
  revision. Its subscriber hears the current revision first, then the latest.
- A document from another engine version (`9.0.0`) is refused before it is read. From then
  on the worker refuses loads and projections, and the page refuses touched sets, local
  applies, previews, and writes until a reload.

## The derive benchmark at the limits

The document at the limits (3,350,264 bytes), median of 5 runs; reported, not gated. The
browser times each call from the page, so its figures include posting to the worker and back.

| Measure | Native (release) | Browser (Chromium, derive worker) |
|---|---|---|
| read the document and derive it | 391.4 ms | 520.3 ms |
| every derived value to JSON (6,151,226 bytes) | 90.4 ms | 89.0 ms |
| the top canvas level | 36.9 ms | 41.0 ms |
| memory | 194 MiB peak resident | 121 MiB wasm linear memory |

`prove.sh WORK_DIR` prints this table afresh; timings vary with the host's load.
