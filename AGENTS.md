# Cairn: for agents

You are changing Cairn: a feature, a fix, a refactor, tests or CI, documents, or a review. This file is the entry point; everything it references is authoritative.

## Read, in this order, before writing anything

1. `PRD.md`: what Cairn does. The glossary is the vocabulary; every term in code and docs comes from it.
2. `ARCHITECTURE.md`: how it is built. Do not reopen its decisions in passing; changing one is a call of its own, recorded in `decisions/`.
3. `PRACTICES.md`: how code is written and verified.
4. Your brief in `briefs/`, if you are building a planned one, and `decisions/` for your area.
5. `DESIGN.md`, if your change touches the web app. Screens use its tokens and components, never raw colours, sizes or one-off styles.

## Rules for every change

- **Own the work end to end.** Done means `mise run check:fast` passes on the rebased result and you have used the change the way a person or agent would: walked the fixtures through a feature, or reproduced a bug and seen the fix clear it. A failing step is the next thing to fix, not a note in the handoff.
- **Every assertion can fail for a real bug.** A test earns its place by catching a break nothing else catches. Reviewers flag tests that cannot fail: an assertion on a mock or a constant, a tautology, a re-check of what an earlier line implies.
- **Show a change people can see.** A change to a screen is shown by screenshots of every state it touches; look at them as a demanding designer would and fix what is off. `mise run screenshots` takes the states listed in `web/app/e2e/screenshots.ts` (add yours there); a flow comes from a Playwright `*.proof.ts` spec run with `mise run proof <dir>`. Specs, pictures and notes stay in the change's scratch directory, never committed. A derived value or an endpoint is shown in the review request with a few sentences or a small table of real fixture values.
- **No stubs.** Build only against code that exists. If your work needs something missing, build it first.
- **No use-case content.** Follow the PRD's domain-free rule in everything you write.
- **Cite requirement ids in code only.** A function implementing a requirement names its id in the doc comment; a scenario test names the ids it covers. Ids and the names of the root documents never appear in text a person or agent reads (screens, messages, tool and schema descriptions, the OpenAPI document, the served guide): say the rule in words; decision records do the same. Above a doc comment that becomes a generated description, cite in a plain `// PRD: A2.` comment instead. Tests fail on an id or document name in served text or on a screen.
- **Earlier work is not frozen.** Change what your work needs; existing tests keep passing unless your change is to the behaviour they check. Concurrent work happens in separate jj workspaces (`mise run workspace <name>`), and whoever lands second reconciles.
- **The documents win.** If a brief, a decision, or the code contradicts a root document, follow the document and record the call in `decisions/`. The PRD wins on what, ARCHITECTURE on how, PRACTICES on code and checks, DESIGN on how screens look. A change that alters what a document says rewrites that line in the same commit. Guidance is written as principles, said once in the document that owns it.
- **Write current state, never history.** Documents, decisions, briefs and comments say how things are now; how they came to be, what they replaced, and what introduced them belong to git. When a call changes, rewrite or delete the decision it replaces.
- **Decide, record, keep going.** The owner is often away, so do not stop to ask. Calls that shape the design, settle what a requirement means, or touch an invariant or limit go in `decisions/`, one file each (`decisions/README.md`). Smaller calls need no record beyond the code.
- **Simple over clever, limits not knobs**: PRACTICES' Simplicity, Scope and dependencies, and Explicit limits. A limit you hit is recorded in `decisions/` with the evidence.
- **Naming.** `route` is the process template; a framework's route is qualified or called by its type name.

## Review and landing

- One logical commit per unit of work: the change with its tests, documents, and review fixes.
- Before it lands, an independent reviewer reviews the whole change (`mise run review`). Verify each finding against the code, fix the valid ones, and record one that changes a document or a design call like any other call. A second round reviews only the fixes, when the first found something severe, several medium findings, or the fixes were large.
- Land onto current main with `mise run land`: it rebases, runs `check:fast`, and moves main. While main's newest CI run is red it takes only a fix. Once a unit lands, push main, move the default workspace to the new main when its working copy is empty, and remove the unit's workspace; do not park finished work.

## Repository conventions

- Version control is jj, not git. Commit only your own files.
- A recurring agent job is a skill in `.agents/skills/`.
- Commit messages: a short imperative title (at most 72 characters), and a body wrapped at 72 columns only when the title leaves the why unsaid. No verification runs, test names, timings, sizes, review notes, lists of design calls, or internal ids. A commit adding a dependency says why in one line.
- ASCII punctuation in documents and comments.

## Talking with people

- **Plainly.** Give the current state, not its history; define a term the first time, or pick a plainer one; check a status live before reporting it; for a call about a screen, show the screen.
- **On the merits.** Weigh every proposal, the owner's included, recommend one, and push back with the concrete tradeoff. A position moves for a better argument, never for insistence or work already done. A question asks for an answer, not a change.

## When you are unsure

Prefer the PRD's wording over your reading of it. Prefer a rejection with a full violation list over a partial success. Prefer an assertion over a comment.
