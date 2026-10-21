# The in-browser tests run on the built demo site

Question: the browser step served the in-browser host from Vite's dev server, which
transforms and serves the app's modules one request at a time with development React. That
made each page load cost a second or more, and about 160 page loads a run. Should the tests
run on something faster, and what does that change for proofs?

## The call

The in-browser host is the demo site as shipped: `mise-tasks/build/web demo` builds it once
per run (in `scripts/e2e-servers`, beside the binary's start) and `vite preview` serves it
on the config's demo port. Every in-browser spec, and every proof run, loads the bundled,
minified app. Documents that named the dev server (the proof rule in the agent guides, the
validation ladder, the step and config comments) say the built demo site instead.

## Alternatives

- Keep the dev server and cut page loads only. Rejected: the loads are the cost, and the
  tests would still run an app unlike the one that ships.
- Serve the built bundle with a hand-written static server. Rejected: `vite preview` already
  answers every path with the page, as the demo site's host does.

## What would change it

A bug that only the dev server's unbundled modules expose, or a build of the demo site that
costs more than the page loads it saves.
