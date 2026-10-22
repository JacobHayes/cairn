# Releases run from a dispatch

Question (from making deploys scriptable): a release was a pushed `v*` tag, which re-ran the whole check on a commit main had already checked, and a script on the host cannot push a tag with the workflow token (a tag pushed with `GITHUB_TOKEN` starts no workflow). How is a release cut, and what keeps it to a commit that passed?

Call: a `release` workflow (`.github/workflows/release.yml`), run by hand with a `version` input on main. It holds the `package` and `release` jobs that used to be in `check.yml`, which no longer runs on tags (`on.push` is limited to branches), so nothing releases from a pushed tag.
- `verify` refuses unless the run is on main, a completed `check.yml` push run on main at the dispatched commit succeeded (no run, or a failed lookup, refuses), and the tag is absent or already on that commit.
- `package` builds that commit with `CAIRN_VERSION` set to the input; `build.rs` stays the one semantic-version check, so a bad version fails before any tag exists.
- `release`, the only job with write access, re-reads the tag and refuses one on another commit before uploading, then `gh release create --target <commit>` makes the tag itself. Uploading to an existing release (a re-run at the same commit) stays.
- A concurrency group per version, never cancelling, keeps two dispatches of one version from overlapping.
- If a workflow file changes on main while a release runs, GitHub can refuse a `GITHUB_TOKEN` tag at an older commit whose workflows differ. The run fails loudly and the fix is a fresh dispatch; re-runs are not promised to recover it.

Alternatives: a personal token or app that can push tags (a credential to store and rotate, for no gain); dispatching `check.yml` on the tag (re-runs the whole ladder); refusing whenever main has advanced since the dispatch (the landing loop moves main often, so most releases would fail).

What would change it: a deployment that releases from branches other than main, or an app credential already kept for other reasons.
