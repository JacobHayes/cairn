# The check sweeps the testbeds; the nightly run sweeps more seeds

Question: the testbeds' sweeps ran nightly, outside the gate, until a sweep ran in seconds. Now that one does, where do they run?

Call: in the check, as the sim step. Each testbed has one sweep, every fault class it is meant to meet in one configuration (its `patina.toml`), and the sweep's exit status is the verdict. The step sweeps 256 seeds, in `check:fast` whenever a Rust input changes and in CI on a part of its own (an `ubuntu-24.04` runner set up for patina's sandbox); CI's nightly run sweeps 2048, as many as fit well inside the job's time limit. On a 24-CPU host the two sweeps take 12 s; the testbeds' release builds take about 45 s after a change to the service and 20 s after one to a testbed; a step with nothing changed takes a second, since a sweep of the same binary resumes; 4096 seeds take three minutes. On CI's four-CPU runner the sweeps take two minutes, and the builds seven with no cache. A new site only a second writer can reach is listed as out of reach in the single-client testbed's `patina.toml`.

One sweep per testbed rather than a leg per fault class: a run with every class at once reaches what needs two together (a crash after a lost acknowledgement was resent needs both buggify and crashes), and each site's coverage is judged once. No leg checks patina's own determinism: patina replays each finding's first seed and says whether it repeated.

Alternatives: keep the sweeps nightly (a break shows up a day later); debug builds (the sweeps run three times slower).

What would change it: the step past a minute with its builds warm, or a sweep that fails CI on a seed it passes locally.
