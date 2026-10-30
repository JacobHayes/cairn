# Step lines time building apart from testing

Question: one number per step says nothing about how much was compiling and how much testing, and a time limit on it compares a cold build with a slow test. How does each line separate the two?

Call: each span is timed where it runs, never by subtraction. `scripts/timed build|tests COMMAND...` wraps every compile site and test run and appends one span to the step's times file; with no step collecting it only runs the command. A kind counts the wall time something of that kind was running, so overlapping builds count once. The line reads like `web-unit: passed, 1430 tests, 38s tests, 90s build, 140s in all`; the whole time stays because some work (lint, the network) is neither. Doc tests count as tests. CI reports whether the Rust dependency cache was an exact, partial or no hit, since our own crates compile in every case.

Alternatives: subtracting test time from step time (builds overlap tests, so the difference is neither); summing overlapping spans (parallel builds would read as many times the wait); parsing cargo's timings (covers cargo only).

What would change it: a step budget that needs more than two kinds, or a compile site that cannot be wrapped where it runs.
