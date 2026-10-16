# The MCP server reports the binary's version

Question (from the release work): MCP `serverInfo.version` was the crate version. With the crate frozen at `0.0.0`, where does it come from?

Call: from the host. `cairn_mcp::router` takes the version as an argument, `cairn_api::router_beside` passes it through, and the composition root's `Assembly` carries it: `serve` fills it with `cairn::VERSION`, the build's version. The API's test-facing routers and the integration support pass `dev`. A test can push a distinct version (`1.2.3-test`) through the root, the API and MCP and read it back from `initialize`, which a default build, where everything is `dev`, never could.

Alternatives: read `cairn::VERSION` inside `root::app` (no field, but then no test can show the version travels); a field on the service's `Parts` or capabilities (it would put the version in the service and on the wire: the capabilities schema, the OpenAPI document and the TypeScript client).

What would change it: a second host that assembles the server and reports a version of its own.
