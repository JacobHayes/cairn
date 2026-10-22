# Releases run from a dispatch

Question (from making deploys scriptable): a release was a pushed `v*` tag, which re-ran the whole check on a commit main had already checked, and a script on the host cannot push a tag with the workflow token (a tag pushed with `GITHUB_TOKEN` starts no workflow). How is a release cut, and what keeps it to a commit that passed?

Call: a `release` workflow (`.github/workflows/release.yml`), run by hand on main with a `bump` input (`major`, `minor` or `patch`, no default) and no typed version: `scripts/next-version` takes the highest plain `vX.Y.Z` tag, bumps that part and resets the lower ones, so a release cannot be mistyped or skip a number. A pre-release is not cut by the workflow. It holds the `package` and `release` jobs that used to be in `check.yml`, which no longer runs on tags (`on.push` is limited to branches), so nothing releases from a pushed tag.
- `verify` refuses unless the run is on main, a completed `check.yml` push run on main at the dispatched commit succeeded (no run, or a failed lookup, refuses), and the tag is free (the computed version is its output).
- `package` builds that commit with `CAIRN_VERSION` set to the computed version; `build.rs` stays the one semantic-version check.
- `release`, the only job with write access, re-reads the tag and refuses one on another commit before uploading, then `gh release create --target <commit>` makes the tag itself. A failed run is never re-run (`verify` refuses attempt 2 and later, since a re-run would pick its version from the tags of that moment); a fresh dispatch recovers it, and `gh release create` fails on a release that exists.
- One concurrency group, never cancelling, keeps two dispatches from computing the same version.
- If a workflow file changes on main while a release runs, GitHub can refuse a `GITHUB_TOKEN` tag at an older commit whose workflows differ. The run fails loudly and the fix is a fresh dispatch; re-runs are not promised to recover it.

Alternatives: a personal token or app that can push tags (a credential to store and rotate, for no gain); dispatching `check.yml` on the tag (re-runs the whole ladder); refusing whenever main has advanced since the dispatch (the landing loop moves main often, so most releases would fail).

What would change it: a deployment that releases from branches other than main, or an app credential already kept for other reasons.
