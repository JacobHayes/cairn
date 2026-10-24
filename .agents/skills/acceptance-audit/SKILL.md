---
name: acceptance-audit
description: Check a change against the plan or brief it was built from - every item done, done differently, or not done, with evidence from the code - and find what the change quietly weakened. Use before a change lands (mise run review --plan) or on landed commits, given both ends of the change. Read-only.
---

# Acceptance audit

A builder's report is a claim, not evidence. Read the plan or brief you were given and the change between the two revisions you were given, and account for everything the plan asks for: its acceptance, tests, documents, proof and deletions. Where the plan conflicts with the governing documents AGENTS.md names, the documents win; say so.

For each item say **done** (where, and whether there is sign it ran), **done differently** (what instead, and whether it serves the same goal), or **not done**. Judge from the code, not the commit message.

Then look for what the change weakened without saying so: lost coverage, a dropped assertion, a skipped check, a behaviour that used to hold. A deleted test weakens only if what it caught is now caught nowhere. List every deferral with its reason.

Open with a table of items and verdicts, then each gap with what would close it. Change nothing.
