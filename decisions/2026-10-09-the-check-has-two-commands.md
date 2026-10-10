# The check has two commands and named steps, not numbered rungs

Question (from the redesign's landing protocol, which needed the whole browser suite on every landing): the ladder had seven numbered rungs, each its own task, but people only ever ran two things, the inner loop and the whole check. Should the rungs stay a user-facing concept?

Call: no. `mise run check:fast [filter]` and `mise run check` are the only commands; the steps (`scripts/steps/`: lint, unit, web-unit, generated, property, integration, cost, browser) are named in the output and keep a log each (`dist/ladder/<step>.log`). Each command runs the fail-early steps in order (lint, then the unit tests), then the slow independent steps side by side. Unfiltered `check:fast` adds the browser step when a web input changed (about 40 s, so the landing protocol needs nothing more), and property when a Rust input did; `check` adds integration, cost, and generated freshness. The generated step stays in the one-at-a-time part because it deletes and rewrites files the other steps read (the schema, the OpenAPI document, the generated types).

Alternatives: keeping `check:N` for rerunning one rung (a filter or the step's log gives the same, and the numbers meant a table to keep in sync); running generated side by side (a race with every step reading its files); leaving the browser suite out of `check:fast` (landing would run the slowest command for what the fast one covers in under a minute).

What would change it: the browser step growing past a minute in `check:fast` (move it to `check` and have landing run that), or a step whose failure is only worth seeing after the others finish.

Older briefs and decisions use the old numbers: 1 lint, 2 unit, 3 property, 4 integration, 5 generated, 6 web-unit and browser, 7 cost.
