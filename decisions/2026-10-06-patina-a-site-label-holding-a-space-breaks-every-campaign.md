# [patina] a site label holding a space breaks every campaign over the binary

- Question: why did the first multiplayer campaign stop at generation 0 with `malformed SDK declared-site token "client:"`?
- Call: `PATINA_SDK_REPORT` writes its `declared_site=` and `site=` rows space-separated without escaping (verdict labels are escaped), and every literal-label site is declared at link time, so one label holding a space anywhere in the binary fails every campaign, reached or not. Cairn's own oracle labels (4.1's service write path, 4.2's client retry) were sentences; they are renamed in kebab-case (`service-patch-lost-at-commit`, `client-stale-patch-retried`, ...), the workaround, and every new label follows. Reproducer: `testbeds/multiplayer/gaps` (`spaced_label`); `sim.sh` fails once it stops reproducing.
- Alternatives: running campaigns with oracles waived (loses the coverage gate); keeping sentence labels for readability (no campaign could run).
- What would change it: patina escaping site labels in the SDK report; sentence labels would then be allowed again, though kebab-case keeps them grep-able.
