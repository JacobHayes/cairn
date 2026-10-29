# Cairn: for agents

You are changing Cairn: a feature, a fix, a refactor, tests or CI, documents, or a review. This file is the entry point; everything it references is authoritative.

## Read, in this order, before writing anything

1. `PRD.md`: what Cairn does. Its glossary is the vocabulary of code and docs; its requirement ids (A1, D1a, F5) are how code cites it.
2. `ARCHITECTURE.md`: how it is built. Do not reopen its decisions in passing; changing one is a call of its own, recorded in `decisions/`.
3. `PRACTICES.md`: how code is written and verified.
4. `DESIGN.md`, if your change touches the web app. Screens use its tokens and components, never raw colours, sizes or one-off styles.
5. `decisions/` for your area, and your brief in `briefs/` if you are building a planned one: build what it scopes, meet its acceptance, append small calls to its Decision log, and delete the brief in the commit that lands it.

## Rules for every change

- **Own the work end to end.** Done means `mise run check:fast` passes on the rebased result and you have exercised the change the way it is used: walked the fixtures through a feature, or reproduced a bug and seen the fix clear it. Use `mise run check:fast <filter>` while coding (the matching Rust tests, Vitest files and Playwright spec) and `mise run check:fast` after every change; CI runs the full `mise run check` on every push of main and is the gate (PRACTICES, The validation ladder). A failing step is the next thing to fix, not a note in the handoff.
- **Every assertion can fail for a real bug.** A test earns its place by catching a break nothing else catches (a fix's test fails without the fix). Reviewers flag tests that cannot fail: an assertion on a mock or a constant, a tautology, or a re-check of what an earlier line implies. No bug needs to be planted to show it.
- **Show a change people can see.** A change someone can see or use (a screen or flow, an endpoint, a command or setting, a value Cairn derives) is shown by its screenshots, judged by the screen critique in review. Picture every state the change touches, look at them as a demanding designer would, and fix what is off. `mise run screenshots` takes every state in the table in `web/app/e2e/screenshots.ts` (add yours there) into `dist/screenshots/`, or into `CAIRN_SCREENSHOTS_DIR=<dir>`; a flow, or a state the table lacks, comes from a Playwright `*.proof.ts` spec run with `mise run proof <dir>` against `cairn demo` (the spec takes `test` from `web/app/e2e/test.ts`, which starts one for each test; the directory holds a `package.json` of `{"type":"module"}`). Specs, screenshots, videos and notes stay in the change's scratch directory; nothing is committed for them. A derived value or an endpoint is shown in the review request by a few sentences or a small table of real fixture values.
- **No stubs.** Build only against code that exists. Nothing returns "not implemented", no pass is an empty slot, and no test passes because the thing it checks is missing. If your work needs something that does not exist, build it first, or record it in `decisions/` and work on what you can.
- **No use-case content.** Follow the PRD's Domain-free requirement in everything you write.
- **Cite the PRD, in code only.** A function implementing a requirement names its id in the doc comment; a scenario test names the ids it covers. Ids and the names of the root documents (PRD, ARCHITECTURE, PRACTICES, DESIGN, AGENTS, README) never appear in text a person or agent reads: the UI, violation and error messages, tool and schema descriptions, the OpenAPI document, the served guide and workflows, and decision records say the rule in words. Where a doc comment becomes a generated description (schemars, OpenAPI, MCP schemas), put the citation in a plain `// PRD: A2.` comment above it. The `served_text` integration tests and every browser test fail on an id or document name in served text.
- **Earlier work is not frozen.** No change owns code. Change what your work needs; existing tests keep passing unless your change is to the behaviour they check. Concurrent work happens in separate jj workspaces, and whichever lands second reconciles what both touch.
- **The documents win.** If a brief, a decision, or the code contradicts `PRD.md`, `ARCHITECTURE.md`, `PRACTICES.md`, or `DESIGN.md`, follow the document and record the call in `decisions/`. The PRD wins on what, ARCHITECTURE on how, PRACTICES on code, limits and the ladder, DESIGN on how screens look. A change that alters what a document says rewrites that document's line in the same commit.
- **Write current state, never history.** Documents, decisions, briefs and comments state the rule and how things are, once, in the document that owns it: the goal first and in the positive, with a specific preference as an example under its principle, in the fewest plain words. No narrative of how or why something came to be, what it replaced, was renamed from, or used to do, and no links to the decision or work that introduced it. History is git's job. When a call changes, rewrite or delete the decision it replaces.
- **Decide, record, keep going.** The user is often away, so do not stop to ask. When the documents are silent, ambiguous, or disagree, follow the precedence above, make the call, and continue. Calls that shape the design or architecture, settle what a requirement means, or touch an invariant or limit go in `decisions/` (`decisions/README.md` says how). Smaller calls (names, layouts, library picks within the rules) need no record beyond the code, or a line in the brief's Decision log.
- **Simple over clever.** Do as much as necessary and as little as possible (PRACTICES, Simplicity).
- **Limits, not knobs.** No new configuration. Limits live in `PRACTICES.md`. A limit you hit is a design signal: record it in `decisions/` with the evidence and work within it; change the limit in `PRACTICES.md` and `limits.rs` only after the user signs off.
- **Naming.** `route` is the PRD's process template. Where code needs another sense of the word (an HTTP or navigation route), qualify it or use the framework's type name. Big-endian names with units, no abbreviations beyond the PRD's.

## Review and landing

- One logical commit per unit of work: the change with its tests, documents, and review fixes folded in.
- Before it lands, an independent reviewer reviews the whole change. Verify each finding against the code, fix the valid ones, and record any that would change a document or a design call in `decisions/`. A second round reviews only the fixes, and runs only when the first found something severe, several medium findings, or the fixes were large; a third, only when the second found something severe.
- Land onto current main: rebase, check for conflicts with what landed meanwhile, and pass `mise run check:fast` on the result. A release is tagged only on a commit whose CI run passed, and while the newest finished run on main is red, `mise run land` takes only the fix (`--fix`). On main a newer push waits for the run in flight rather than cancelling it, so the newest run covers everything pushed before it.
- Integrate as you go: once a unit passes, land it, push main, move the default workspace to the new main when its working copy is empty, and remove the unit's workspace. Do not park finished work or ask before landing it.

## Repository conventions

- Version control is jj, not git. Commit only your own files.
- Instructions for a recurring agent job live once, as a skill in `.agents/skills/` (`.claude/skills` links there); tasks and workflows run them.
- Commit messages: a short imperative title (at most 72 characters, aim for 50-60) saying what changed, and a body wrapped at 72 columns only when the title leaves the why unsaid. No verification runs, test names, timings, sizes, review notes, lists of design calls, or internal ids (brief, requirement, review-round, ticket). A commit that adds a dependency says why in one line.
- Generated files live only in the generated paths (ARCHITECTURE, Generated artifacts), are marked generated, and are never edited by hand: regenerate with `mise run gen` and commit them with the source that changed them.
- ASCII punctuation in documents and comments: plain hyphens, straight quotes.

## Talking with people

- **Plainly.** Give the current state, not its history; define a term the first time you use it, or pick a plainer one; check a status live before reporting it; for a call about a screen, show the screen or a mockup of each option.
- **On the merits.** Weigh every proposal, the user's included, recommend one, and push back with the concrete tradeoff when you disagree; a position moves for a better argument, never for insistence or for work already done. A question asks for an answer, not a change.

## When you are unsure

Prefer the PRD's wording over your reading of it. Prefer a rejection with a full violation list over a partial success. Prefer an assertion over a comment.
