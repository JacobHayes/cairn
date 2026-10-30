# The capabilities document names the server's release

Question: a person using the web app needs to tell which release the deployment runs, which only `cairn version` and the MCP `serverInfo` otherwise say. Where does the page get it?

Call: the capabilities document carries `version`, the binary's own version string (a release such as `0.13.0`, or `dev` for a build without a release tag), and the sync popover shows it at the right of its head (`v0.13.0`, or `dev build`). The web build and the wasm module carry no version, so a release changes nothing committed and version skew detection stays per web build.

Alternatives: stamp the version into the web build (every release would change the built page); a separate version endpoint (one more read for a fact the page already asks for at start).

What would change it: a page served apart from any server, which would need the version in its own build.
