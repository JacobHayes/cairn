# The browser step runs one worker per core on CI

Question: Playwright's default is half the logical cores as workers, which is two on a
four-core runner. With the browser tests culled and the server tests parallel, is that still
the right count for CI?

## The call

`workers: "100%"` when `CI` is set; the default locally. Measured on the culled suite pinned
to four cores (`taskset -c 0-3`), three runs each: two workers took 101 s on every run, four
workers 76 to 77 s, a quarter faster, with no failure in any run. The in-browser tests are
light on the machine's cores (the page's wasm does the work in the browser process), so the
four workers overlap waits for the page rather than fight over the cores.

## Alternatives

- Keep the default. Rejected: it left about a quarter of the step's time on the table.
- More workers than cores. Not measured: the server tests share one binary and the serial
  group is one worker, so the next limit would be the binary, not the cores.

## What would change it

A flake that only appears at four workers, or a CI runner with a different core count.
