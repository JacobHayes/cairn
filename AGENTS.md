# Cairn: for agents

You are changing Cairn: a feature, a fix, a refactor, tests or CI, documents, or a review. This file is the entry point; it is short on purpose and everything it references is authoritative.

## Read, in this order, before writing anything

1. `PRD.md`: what Cairn does. The glossary is the vocabulary; every term in code and docs comes from it. Requirement ids (A1, D1a, F5) are how everything downstream cites it.
2. `ARCHITECTURE.md`: how it is built. Crate boundaries, the pure engine, the store, the hosts. Do not reopen its decisions in passing; changing one is a call of its own, recorded in `decisions/`.
3. `PRACTICES.md`: how code is written and verified. Assertions, limits, the validation ladder.
4. `briefs/README.md`, then the briefs that built what you are changing, and your brief if you are building a planned one. The landed briefs are the record of the milestones that built Cairn, each with its decision log and its proof under `briefs/proof/`; read them for why the code is the way it is. A planned brief is a unit of work: build what it scopes, meet its acceptance, append small calls to its Decision log, leave its proof under `briefs/proof/<id>/`, and mark it landed in the README's table in the same commit. Work no brief covers starts from the documents alone. `decisions/` holds every call that matters, from the briefs and since; check it for your area too.
5. `DESIGN.md`, if your change touches the web app: the visual language (tokens, type, layout, components, charts). Screens use its tokens and components, never raw colours, sizes or one-off styles.

## Rules for every change

- **Own the work end to end.** Done means `mise run check` passes in full and you have exercised the change yourself the way it is used: walked the fixtures through a feature, or reproduced a bug and seen the fix clear it. `mise run check:fast <filter>` while coding (the matching Rust tests, Vitest files and Playwright spec), `mise run check:fast` after every change, the full `mise run check` before landing (PRACTICES, The validation ladder). A failing rung is the next thing to fix, not a note in the handoff.
- **Every assertion can fail for a real bug.** A test earns its place by catching a break nothing else catches, and each of its assertions fails when the behaviour it checks is wrong (a fix's test fails without the fix). Reviewers flag tests that cannot fail: an assertion on a mock or a constant, a tautology, or a re-check of what an earlier line already implies. No bug needs to be planted to show it.
- **Leave proof people can see.** A change someone can see or use (a screen or flow, an endpoint, a command or setting, a value Cairn derives) leaves a proof page at `briefs/proof/YYYY-MM-DD-<short-slug>/README.md` (for a brief, `briefs/proof/<id>/README.md`), committed with the work. Internal changes (a fix that restores intended behaviour, a refactor, tests, CI, documents) leave none; their tests and commit message carry the evidence. A proof follows the brief proofs' format: a page a person takes in at a glance, about 60 lines at most, opening with what someone can now do or see. A screen embeds the Playwright screenshots of its distinct states and a short video of the main flow, each with a one-line caption, saved beside the README; engine or server work shows a small before-and-after table of real values from a fixture, or a few plain sentences of observed behaviour. It ends with known limits, if any, in one to three bullets. Requests, outputs, and test names stay in the tests and the commit. A script beside the README may regenerate its media or value table; keep media small (PNG, short WebM). Earlier proofs record what their milestone shipped; leave them as they are.
- **No stubs.** Build only against code that exists. Nothing returns "not implemented", no pass is an empty slot, and no test passes because the thing it checks is missing. If your work seems to need something that does not exist, build that first, or record it in `decisions/` and work on what you can.
- **No use-case content.** Follow the PRD's Domain-free requirement (Non-functional requirements) in everything you write, even when a conversation about Cairn mentions a specific use case.
- **Cite the PRD.** A function that implements a requirement names its id in the doc comment; a scenario test names the ids it covers. Reviewers check coverage this way.
- **Earlier work is not frozen.** No brief or earlier change owns code. Change what your work needs; existing tests keep passing unless your change is to the behaviour they check, and the commit body says so. Work running at the same time happens in separate jj workspaces, and whichever lands second reconciles anything both touch, including shared fixtures and test suites.
- **The documents win.** If a brief, a decision record, or the code contradicts `PRD.md`, `ARCHITECTURE.md`, `PRACTICES.md`, or `DESIGN.md`, follow the document and record the call in `decisions/`. Between the documents, the PRD wins on what, ARCHITECTURE on how, PRACTICES on code, limits, and the ladder, DESIGN on how screens look. A change that alters what a document says updates the document in the same commit.
- **Decide, record, keep going.** The user is often away, so do not stop to ask. When the documents are silent, ambiguous, or disagree, follow the precedence above, make the call, and continue. Calls that matter go in `decisions/` at the repository root for the user to review later: anything that shapes the design or architecture, settles what a PRD requirement means, or touches an invariant or limit. One new file each, `decisions/YYYY-MM-DD-<short-slug>.md`: a `# title`, then the question (naming the work it came from), the call, the alternatives, and what would change it (`decisions/README.md`). Everything smaller (names, layouts, library picks within the rules) goes in the commit body, and for a brief also in its Decision log.
- **Simple over clever.** Do as much as necessary and as little as possible: question a layer before optimizing it, fail closed and reopen from durable state, migrate old data once instead of branching on it, keep no deprecated aliases nobody depends on, and prefer plain and portable (PRACTICES, Simplicity).
- **Limits, not knobs.** No new configuration. Limits live in `PRACTICES.md`. A limit you hit is a design signal: record it in `decisions/` with the evidence and keep working within the limit; change the limit in `PRACTICES.md` and `limits.rs` only after the user signs off.
- **Naming.** `route` is the PRD's process template. Where code needs another sense of the word (a framework's HTTP or navigation route), qualify it or use the framework's own type name, so a bare `route` always means the template. Big-endian names with units, no abbreviations beyond the PRD's.

## Review and landing

- One logical commit per unit of work: the change with its tests, documents, proof, and review fixes folded in.
- Before it lands, an independent reviewer reviews the whole change. Verify each finding against the code, fix the valid ones, and record any that would change a document or a design call in `decisions/`.
- A second round reviews only the fixes, and runs only when the first found something severe, several medium findings, or the fixes were large. A third, again only the fixes, runs only when the second found something severe.
- Land onto current main: rebase, check for conflicts with what landed meanwhile, and pass `mise run check` on the result. CI runs the full check on every push; a newer push supersedes an older run, so the newest run covers everything pushed before it.

## Repository conventions

- Version control is jj, not git. Commit only your own files.
- Commit messages: a short imperative title (at most 72 characters, aim for 50-60), a blank line, then a body wrapped at 72 columns saying what changed and why, with notable design calls and how it was verified. No internal ids (brief, requirement, review-round, or ticket ids).
- Generated files live only in the generated paths ARCHITECTURE lists (Generated artifacts), are marked as generated, and are never edited by hand. They are regenerated with `mise run gen` and committed in the same change as the source that changed them. Hand-written code sits beside them, never inside them.
- A new dependency is justified in one line in the commit message that adds it (PRACTICES, Dependencies are deliberate).
- ASCII punctuation in documents and comments: plain hyphens, straight quotes.

## When you are unsure

Prefer the PRD's wording over your reading of it. Prefer a rejection with a full violation list over a partial success. Prefer an assertion over a comment. If two documents disagree, follow the precedence above and record the call in `decisions/`.
