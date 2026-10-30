# CI caches survive failed runs, and a main run in flight is never cancelled

Question: a cache saved only from a passing, uncancelled run is rarely saved, since main's runs are often superseded or red. How do the caches get saved?

Call: on main a run in flight finishes; a newer push waits, and only the newest waiting run is kept. Other refs cancel superseded runs. Each part has its own rust-cache entry, saved on failure and only from main, keyed on a hash of the root manifest's profile tables, since rust-cache does not hash the root manifest and a profile change would otherwise hit stale artifacts and never save. Do not cancel a main run mid-build: it can save a partial entry; delete that entry by hand if it happens.

Alternatives: sccache (cannot cache what links, which is every test binary); saving `target/` with `actions/cache` every run (a gigabyte per part per run); never cancelling on any ref (branches queue behind superseded commits); npm and Playwright browser caches (no faster than downloading).

What would change it: a cache provider where one key per run with restore keys costs less than the save-once rule, or main runs routinely cancelled by hand.
