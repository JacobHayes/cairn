# The capabilities document names the server's release

The question, from showing the version in the sync popover: nothing the web app reads carried the version a deployment runs (only `cairn version` and the MCP `serverInfo` did), so a person could not tell which release they were using. Where should the page get it?

The call: the capabilities document carries `version`, the binary's own version string (a release such as `0.13.0`, or `dev` for a build without a release tag), and the sync popover shows it at the right of its head (`v0.13.0`, or `dev build`). The web build and the wasm module carry no version, so a release changes nothing committed and version skew detection stays per web build.

Alternatives: stamp the version into the web build (it would make every release change the built page, and the demo site is built apart from any release); a separate version endpoint (one more read for a fact the page already asks for at start).

What would change it: a version for the demo site, which would need its build to be tied to a release.
