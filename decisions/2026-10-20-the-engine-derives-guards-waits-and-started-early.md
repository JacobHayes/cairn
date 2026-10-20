# The engine derives guards, waits and started early

**Question.** The app decided for itself which guards a done button would trip (an open dependency, a missing artifact or note), what a node waits on (walking its ancestors' gates), whether active work started early, and which decisions an undecided node waits on. Each copy could disagree with the engine. Where do these live? (From moving derived values out of the web app and into the engine.)

**Call.** `NodeDerived` carries them, derived on every read and never stored:
- `failing_guards`: the guards the node's completing transition would fail now (D4), taken from the same checks a patch runs, so it equals the bypassable guards of a refused patch. A completion records a finish date, which can move an auto-reached milestone's date, so a patch may be accepted where a guard is reported; never the reverse.
- `waits_on`: each unsatisfied dependency the node waits on, its own first and then the nearest ancestor's first, once per node, cut to the explanation limit with a total like the other explanations. The app could not expand what an ancestor holds correctly under a force include, so the engine lists it. A node that waits on more than the explanation limit has the rest counted in the total, and the inspector's Connections says "and N more"; the other readers (the Timeline's needs lines, the waiting walkthrough) show the entries they have.
- `started_early`: stored active while a gate it waits on is unmet. An undecided node held only by its condition gate now counts as started early, as the engine's blocking says; the app's copy did not.
- `relevance.unanswered`: the decisions an undecided node waits on.

**Alternatives.** Keep the app's copies with parity tests: keeps the duplication, and a test cannot see a future rule. Send only the final verdicts (a flag per button): loses the reason the screens show.

**What would change it.** A guard whose failure depends on something the engine cannot see (a host-side check), which would stay in the host and not be reported here.
