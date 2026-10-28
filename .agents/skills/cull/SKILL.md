---
name: cull
description: Read-only audit of Cairn, one area of it, or one change, for what it would be better without - one rule kept in several places, duplicated or dead code, unused features, interfaces callers work around, tests that catch nothing, documents that repeat each other, on-screen text that does not earn its place - written up as changes a builder can take. Use after a change or a plan of changes lands, or when asked to cull or audit. Edits nothing.
---

# Cull

Find what Cairn would be better without, and say how to remove it. Change nothing. Read AGENTS.md first; PRACTICES.md's Simplicity section and "Judge what exists" are the standard every finding is held to, and DESIGN.md's principles for screens.

Your scope is what you were given.
- **A change:** start from what it touched and follow each rule it touches to the other places that decide the same thing. One pass.
- **An area or the whole repository:** pass again, each time from an angle the last did not take, until a pass finds nothing new. Report what each pass found.

## What to look for

Anything that adds weight without earning its place: one rule decided in several places that agree only because someone keeps them in step, one job done twice, code and features nothing reaches, interfaces their callers work around, tests that catch nothing another test does not, and documents that repeat another document or write a case where a principle belongs.

On-screen text earns its place by preventing a real mistake (an irreversible delete, someone else's work lost, a refusal that says how to recover) or by labelling a control. Cut the rest: a notice that states what the person just did, a line that repeats a label or button, a disclaimer about something they can undo, an empty state that only says the obvious.

Copies of one rule rarely look alike. Start from each rule's owner and ask who else decides it, across code, scripts and documents, and whether anything forces them to agree.

## Evidence

Confirm each finding at every copy, and drop what you cannot confirm. For tests, use the evidence that exists: CI's per-test browser report, the Rust suite and step times, and the mutation survivors. The weekly mutation job publishes them as run artifacts: `gh run download $(gh run list --workflow mutants.yml --status success --limit 1 --json databaseId --jq '.[0].databaseId') -n web-survivors -n engine-survivors`, each a tab-separated list under a count line (location, status, what was mutated, the tool's id). The web list covers the whole app; the engine list one slice of sixteen, so a run says nothing about the other fifteen. A survivor is a missing assertion, a test that earns nothing, or an equivalent mutant: say which. A mutation result shows that a cut lost nothing; it does not prove it.

## Output

Changes a builder can take one at a time, ordered by what a person or agent sees today. For each: every copy by file and line, whether the copies already disagree, where the one copy should live and what that lets you delete, and size and confidence. End with a build order, overlap with queued work, and a few lines on what you looked at and left out.
