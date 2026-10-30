# Every deploy is a tagged release

**Question.** When is a release tagged, and does a deployment ever run an untagged build?

**Call.** Every deployment runs a tagged release, cut on main's commit after its CI run passes; several landed units can share one release. The version moves by what changed since the last tag (minor for the API, MCP tools, file format, stored data or a feature; patch otherwise), and 1.0 waits for the owner to call the API stable. A build without a tag reports `dev`.

**Alternatives.** Deploying main untagged and tagging occasionally (the running version names no release); a tag per landed commit (most never deployed); calendar versions (say nothing about compatibility, which an API client needs).

**What would change it.** A second deployment on its own release schedule, or crates published to a registry.
