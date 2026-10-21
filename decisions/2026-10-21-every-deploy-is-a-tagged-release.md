# Every deploy is a tagged release

Question (from deploying the internal instance): releases take their version from a git tag, but nothing said when to make one, so deployments ran `git describe` builds such as `0.4.2-6-g42c0b95d7` and a change that broke API clients shipped under a patch-level number. When is a release tagged, and which part of the version moves?

Call: every deployment runs a tagged release; nothing is deployed from an untagged commit. The tag goes on the commit of main being deployed, after its CI run passes, and its number moves by what changed since the last tag:

- Minor (`0.4.x` to `0.5.0`): anything that changes the API, the MCP tools, the route file format or stored data (a migration), or adds a feature. Before 1.0, minor is where breaking changes go.
- Patch (`0.5.0` to `0.5.1`): fixes and polish that change none of those.
- 1.0 waits for the owner to call the API stable.

Several landed units can share one release; the deploy cuts one tag covering all of them. A build between releases still reports its `git describe` version, so `cairn version` names any build exactly.

Alternatives: deploy main untagged and tag occasionally (the status quo: the running version names no release, and its number says nothing about what changed); tag every landed commit (a release per unit, most of them never deployed); calendar versions (say nothing about compatibility, which is what an API client needs).

What would change it: a second deployment that runs releases on its own schedule, or crates published to a registry (see `2026-10-16-releases-take-their-version-from-the-git-tag.md`).
