# Releases take their version from the git tag

Question: bumping the crate version churns the lock file, the OpenAPI document and the wasm glue, so every release would need a regeneration commit. Where does a release's version live?

Call: in the tag. The crate version stays `0.0.0`, and a `const` assertion fails any other value. The build reads `CAIRN_VERSION`, refuses a value that is not a semantic version, and is `dev` when unset, so check and test builds never vary. The version never reaches the web build or the wasm module, so version skew detection does not depend on it. The host passes it to MCP's `serverInfo` as an argument, never through the service or the wire types. The checked-in OpenAPI document says `unversioned`: it describes the API at its commit, not a release.

Alternatives: bumping the crate per release (churns generated files); reading the version inside the server root (no test could show it travels); the release version in the OpenAPI document (a per-release change to a checked-in file).

What would change it: publishing crates to a registry, or serving the OpenAPI document.
