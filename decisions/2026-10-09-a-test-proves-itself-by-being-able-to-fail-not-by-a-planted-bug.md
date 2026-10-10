# A test proves itself by being able to fail, not by a planted bug

Question (from the user's direction on test value and simplicity): every change that added behaviour had to plant a bug, show one of its tests failing, and quote that output in the commit body. It cost a build and a run per change and a line nobody read, and it proved one test while saying nothing about the rest. Should it stay?

Call: no planting ritual. The rule is that every assertion can fail for a real bug, and reviewers flag tests that cannot (an assertion on a mock or a constant, a tautology, a re-check of an earlier line). A fix's test still fails without the fix, by definition. `AGENTS.md` carries the rule; the brief template's Acceptance names a real bug its tests must catch, and the "Planted bug" lines in planned briefs read that way: the bug the tests guard against, not a step to perform.

Alternatives: keep planting for bug fixes only (the fix itself already is the plant), or keep it as optional evidence (an optional step becomes a ritual again).

What would change it: tests that cannot fail getting past review, found later by a mutation run or a bug a passing test should have caught.
