# The server tests run in parallel, except the ones that share the deployment

Question: the browser step ran every `@server` test one after another against one `cairn
demo`, about 66 s of its wall time. Can they run side by side, and how do the tests that
change the whole deployment stay safe?

## The call

- A server test that needs only a journey or a route makes its own (`ownJourney` in
  `e2e/shell.ts`, a route made by `newRoute`), so tests do not meet on the fixture journeys.
  Those run in the `server` Playwright project, fully parallel on the one binary.
- A test that changes what the whole deployment holds (its identities and revision, the
  assistant's script, the signed-in user, the deployment's open proposals, a count of the
  deployment's fetches) is tagged `@serial`. The `server-serial` project runs those with one
  worker, in file order. The parallel `server` project depends on it, so no parallel server
  test starts until the serial ones are done. Playwright starts a dependent project only when
  every project of the phase before it has finished, so the serial project shares the first
  phase with the in-browser tests, which use no server: its one worker is busy from the start
  while the other workers run those, and the serial chain (about 20 s) is no longer a tail
  after them (the first design, serial last, left it as one).
- `scripts/test` (the filtered inner loop) cannot lean on `dependencies`: a project's
  dependencies run whatever the file or title filter, which would drag in every parallel
  server test. It runs the filtered in-browser and parallel server tests in one invocation
  with `--no-deps`, and the `@serial` tests of the selected specs in a second one.
- `scripts/changed` decides that a spec needs the binary by the tag `@server` in its source,
  not by file name.

## Alternatives

- One project, one worker (as before). Rejected: it is the cost.
- A binary per worker. Rejected: the deployment is one thing in the product; the serial group
  is the honest model of the tests that depend on it.
- Retries to absorb collisions. Rejected: a retried test hides the sharing that caused it.

## What would change it

A parallel server test that must read the whole deployment moves to `@serial`; a serial group
that grows large enough to dominate the step is a signal to give those tests their own
deployment.
