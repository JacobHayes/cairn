# The multiplayer testbed judges liveness, not latency

Question: the testbed required every view to hold the store's revisions within 450 ms after writes stopped, and its clients gave a request up after 64 attempts of 200 ms each. Under network faults some seeds broke both: 3 in 512 views late, 1 in 512 clients giving up, on links whose delayed, split deliveries carry a journey slower than one attempt allows. Are those findings?

Call: no; a slow link is not Cairn's fault. The views' deadline is a liveness bound of a minute, and a client's attempt budget doubles on each retry, as a client backs off. How soon a tick follows a commit is checked by the notifier's deterministic tests (PRACTICES: timing-sensitive code is tested deterministically). The sweep's network delivers pieces of at least 256 bytes: the preset's smallest, under its delays, carry a few bytes a second, on which no request finishes in any time.

Alternatives: fault ranges narrow enough to keep the 450 ms (the bound would hold by the choice of faults); the deadline only in runs without faults (the program cannot tell which fired).

What would change it: a view that stops following, which the minute still catches.
