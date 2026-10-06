# Cairn: for implementers

You are implementing one feature brief of Cairn. This file is the entry point; it is short on purpose and everything it references is authoritative.

## Read, in this order, before writing anything

1. `PRD.md`: what Cairn does. The glossary is the vocabulary; every term in code and docs comes from it. Requirement ids (A1, D1a, F5) are how everything downstream cites it.
2. `ARCHITECTURE.md`: how it is built. Crate boundaries, the pure engine, the store, the hosts. Do not reopen its decisions inside a brief.
3. `PRACTICES.md`: how code is written and verified. Assertions, limits, the validation ladder.
4. `briefs/README.md`, then your brief in `briefs/`. A brief is an implementation milestone: what to build next, in what order, and how to show it works. It points into the documents above rather than restating them.

## Rules that apply to every brief

- **Own the work end to end.** Done means `mise run check` passes in full and you have walked the fixtures through your feature yourself. "In full" means every rung that exists at that point: the ladder grows as briefs land (PRACTICES, Growing the ladder), so an early brief never waits on a rung nothing has filled yet. A failing rung is the next thing to fix, not a note in the handoff. `mise run check:fast` after every change.
- **Prove your tests can fail.** At least one test per brief is shown failing against a planted bug, with its failing output noted in one line in the decision log. A brief that ships no production code (the simulation spike, 1.3) instead shows an injected fault visibly changing a run's outcome.
- **Leave proof the user can see.** Every brief ends with `briefs/proof/<brief id>/README.md`, committed with the work: what someone can now do or see that they could not before, shown rather than claimed. For a screen, Playwright screenshots of each acceptance state and a short video of the main flow, saved beside the README. For engine or server work, the fixture walkthrough as concrete before-and-after values (a table of derived dates or ranks, a request and its response). Passing checks and the planted bug are table stakes, not proof; they go in the decision log in a line each. Generate the proof with a script where you can. Keep media small (PNG, short WebM).
- **No stubs.** Build only against code that exists. Nothing returns "not implemented", no pass is an empty slot, and no test passes because the thing it checks is missing. If your brief seems to need something that has not landed, the order is wrong: record it in `DECISIONS.md` and work on what you can.
- **No use-case content.** Follow the PRD's Domain-free requirement (Non-functional requirements) in everything you write, even when a conversation about Cairn mentions a specific use case.
- **Cite the PRD.** A function that implements a requirement names its id in the doc comment; a scenario test names the ids it covers. Reviewers check coverage this way.
- **Earlier work is not frozen.** Briefs are milestones, not owners. A later brief may change code an earlier one wrote when its own work needs it; the earlier brief's acceptance tests keep passing, and the change goes in your decision log. Two briefs running at the same time each work in their own jj workspace, and the coordinator reconciles anything both touch, including shared fixtures and test suites.
- **The documents win over the brief.** If your brief contradicts `PRD.md`, `ARCHITECTURE.md`, or `PRACTICES.md`, the brief is wrong: follow the document and record it in `DECISIONS.md` so the brief is fixed. Between the documents, the PRD wins on what, ARCHITECTURE on how, PRACTICES on code, limits, and the ladder.
- **Decide, record, keep going.** The user is often away, so do not stop to ask. When the documents are silent, ambiguous, or disagree, follow the precedence above (PRD, then ARCHITECTURE, then PRACTICES, then the brief), make the call, and continue. Calls that matter go in `DECISIONS.md` at the repository root for the user to review later: anything that shapes the design or architecture, settles what a PRD requirement means, or touches an invariant or limit. One entry each, newest first: date, brief, the question, the call, the alternatives, and what would change it. Everything smaller (names, layouts, library picks within the rules) goes in your brief's Decision log section and nowhere else.
- **Limits, not knobs.** No new configuration. Limits live in `PRACTICES.md`. A limit you hit is a design signal: record it in `DECISIONS.md` with the evidence and keep working within the limit; change the limit in `PRACTICES.md` and `limits.rs` only after the user signs off.
- **Naming.** `route` is the PRD's process template. Where code needs another sense of the word (a framework's HTTP or navigation route), qualify it or use the framework's own type name, so a bare `route` always means the template. Big-endian names with units, no abbreviations beyond the PRD's.

## Repository conventions

- Version control is jj, not git. Commit only your own files.
- Commit messages: one line saying what and why, wrapped body if needed.
- Generated files live only in the generated paths ARCHITECTURE lists (Generated artifacts), are marked as generated, and are never edited by hand. They are regenerated with `mise run gen` and committed in the same change as the source that changed them. Hand-written code sits beside them, never inside them.
- A new dependency is justified in one line in the commit message that adds it (PRACTICES, Dependencies are deliberate).
- ASCII punctuation in documents and comments: plain hyphens, straight quotes.

## When you are unsure

Prefer the PRD's wording over your reading of it. Prefer a rejection with a full violation list over a partial success. Prefer an assertion over a comment. If two documents disagree, follow the precedence above and record the call in `DECISIONS.md`.
