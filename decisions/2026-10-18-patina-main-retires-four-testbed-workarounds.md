# [patina] Patina main retires four testbed workarounds

- Question: which testbed workarounds does the Patina bump make unnecessary, and what of their reasons survives (patina-bump)?
- Call: remove all four, each because its record's own "what would change it" happened.
  - The ticker task and `--tick-ms` (`2026-10-06-patina-a-delayed-tcp-segment-does-not-wake-epoll-wait.md`): a reactor park now ends at the next simulated delivery (`0115ebf0`); the faulted sweep passes 8 of 8 with no ticker, every net knob applied.
  - The symbol allowance and `unsupported-symbols.txt` (`2026-10-06-patina-turso-links-symbols-the-shim-refuses-to-run-beside.md`): the audit decodes AVX-512 FP16 and deny-traps `dlopen` (`6bca33af`); the audit exits 0 and no run names a symbol. A new unsupported symbol now fails the audit leg instead of hiding behind a list.
  - `PRAGMA temp_store = MEMORY` in the migrations and its store test (`2026-10-16-a-fresh-store-opens-without-a-temporary-directory.md`): Patina crashes after a removal (`f0d1fc31`); the durability crash sweep restarts and completes 112 of 112 runs without the pragma, and every crash oracle fires. Production consequence: a fresh store's migrations again use Turso's temporary directory for `DROP TABLE`, so opening a fresh store needs a usable `TMPDIR`, as later statements already did.
  - The label rule's reason (`2026-10-06-patina-a-site-label-holding-a-space-breaks-every-campaign.md`): site labels are escaped in the report now. The rule stays, re-grounded: literal, because a campaign gates the literal labels declared at link time; kebab-case, because each testbed's `sim.sh` word-splits and greps labels, so a spaced label would break the scripts.
- Alternatives: keep a workaround as insurance (code kept for a bug that no longer exists, with nothing to fail if it returns); drop the label rule (the scripts still depend on it).
- What would change it: a regression in any of the four at a later pin; the sim legs fail and the matching workaround returns with a record.

Supersedes the four records named above in the parts that describe a workaround.
