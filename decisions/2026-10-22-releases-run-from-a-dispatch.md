# Releases run from a dispatch

Question: a pushed tag re-runs the whole check on a commit main already checked, and a tag pushed with the workflow token starts no workflow. How is a release cut, and what keeps it to a commit that passed?

Call: the `release` workflow runs by hand on main with a `bump` input (major, minor or patch, no default) and no typed version; `scripts/next-version` bumps the highest plain version tag, so a release cannot be mistyped or skip a number. It refuses unless the dispatched commit is on main and its `check.yml` push run succeeded, and unless the tag is free. Only the final job has write access; it re-reads the tag and makes it with `gh release create --target <commit>`. Nothing releases from a pushed tag. A failed run is never re-run, since its version would come from the tags of a later moment; a fresh dispatch recovers it. One never-cancelling concurrency group keeps two dispatches from computing the same version.

Alternatives: a personal token or app that can push tags (a credential to store and rotate); dispatching the check on the tag (re-runs the ladder); refusing whenever main has moved since the dispatch (most releases would fail).

What would change it: releasing from branches other than main, or an app credential already kept for other reasons.
