# CI caches survive failed runs, and a main run in flight is never cancelled

Question (from the CI speed work): from Oct 9 to Oct 16 no CI run on main saved its Rust cache, so every run built from an old entry, in effect from scratch, for 37 minutes. rust-cache saved only from a push run on main that passed, and every main run was cancelled by the next push (`cancel-in-progress: true`) or went red (a flaky browser spec). How do the caches get saved, and how does every push on main end up covered by a finished run?

Call:
- `cancel-in-progress` is false on main and true elsewhere (`github.ref != 'refs/heads/main'`). A run in flight on main finishes and saves its caches; a newer push waits for it, and GitHub keeps one waiting run per group, replacing an older waiting one, so main never queues more than one and the newest push always runs. A branch or tag still drops the run a newer push supersedes. The wording to use is "a run in flight is never cancelled", not "every run finishes": waiting runs are replaced, and push, schedule and dispatch are separate groups. The part jobs have `timeout-minutes: 45` (the slowest cold part measured 23 minutes), so a hung job cannot hold main's queue for six hours.
- Swatinem/rust-cache stays (pinned SHA unchanged), one entry per part (`shared-key: check-<part>-<hash of the root Cargo.toml's [profile] tables>`), `cache-on-failure: true`, `save-if` main. A failed part saves its cache: exercised with a planted failure after the build, where the failed rust leg saved its entry and the next run restored it whole and ran at the warm time.
- One entry per part because rust-cache saves a key once: parts sharing a key would leave all but the first to finish out.
- The profile hash is in the key because rust-cache hashes `Cargo.lock` and the member manifests but not the virtual root manifest that holds the build profiles; without it a profile change would exact-hit artifacts built with the old profile, rebuild every dependency on every run, and never save. A `[workspace.dependencies]` feature edit with no lock change still exact-hits a stale entry until the next lock change; since Oct 6 none of the 26 commits that touched the root manifest was such an edit (21 changed `Cargo.lock` too, 4 were profile edits), and Cargo still decides what is fresh, so the cost is speed only. Recovery for any stale or partial entry: `gh cache delete <key>`; the next main run saves a full one.
- Playwright installs with `npx playwright install chromium`, without `--with-deps`: the runner image carries Chrome's libraries and the browser step passes without them; the apt round trip took 12 to 17 s of a 19 to 24 s step, and the install is now 7 s.

Measured and not adopted:
- An `~/.npm` cache keyed on the lock files: restore 1 to 3 s plus `npm ci` 2 s against `npm ci` alone at 5 s. No gain; dropped.
- A Playwright browser cache keyed on the `@playwright/test` version: a restore took 5 to 6 s against a 7 s download. No gain; dropped (the ticket asked for both caches; the measurement says neither pays here).
- No debug info (`CARGO_PROFILE_DEV_DEBUG=false` in the part environment): cold test build of the rust part 900 s against 983 to 1040 s with it, warm 689 s (browser 849 s) against 540 to 740 s (611 to 894 s). Inside the 25% run-to-run spread, so not adopted, and the local profile is untouched either way.

A run cancelled by hand (exercised on a scratch branch, four parts, caches from empty):
- Cancelled while a part waited or ran tests, rust-cache's post step ran, uploaded, and the rerun restored the entry whole with the warm build time (rust 542 s). `post-if` includes `CACHE_ON_FAILURE`, so it runs on cancel.
- Cancelled while a part was still writing into `target` (the browser leg, in the middle of its Playwright tests), the post step's `tar` failed with "file changed as we read it" and saved nothing; the previous entry, if any, stays. Cancelled in the middle of a build it saves a partial entry (still correct, since Cargo decides freshness) that is not replaced until the key changes (lock file, member manifest, profile, toolchain). With no automatic cancel on main this needs a manual cancel or the timeout, so: do not cancel a main run mid-build, and `gh cache delete` the part's entry if one was cancelled.

Alternatives:
- sccache with the GitHub cache backend: it cannot cache the binary, proc-macro and dylib crates, the build-script runs, or anything that links, which here is every test binary, the proc-macros and Turso's C-ABI libraries; plus thousands of cache entries and rate limits.
- `actions/cache` restore with an `if: always()` save of `target/` per run: re-uploads about a gigabyte per part on every main run and needs our own pruning of workspace artifacts.
- Never cancel, on any ref: runs on branches would queue behind superseded commits.
- `cache-on-failure` off: a red browser spec would again keep that part's cache from ever saving.

Cache sizes (one generation): lint 270 MB, web 440 to 470 MB, rust 484 MB, browser 528 MB; 1.7 GB for the four against the repository's 10 GB, replacing the single 1.1 GB entry. Entries older than 7 days are evicted, and the old `v0-rust-check-...` entries are read by no workflow now.

What would change it: a runner or cache provider that makes rust-cache's save-once rule cost more than it saves (switch to a key per run with restore keys); a main run that is routinely cancelled by hand (add the `tar` retry, or save the cache in a step that runs before the long tests); the workspace crates' artifacts becoming cacheable (see the parts decision), which would make the cache entries larger and the warm build short.
