# The tool list carries the mutation vocabulary once and no output schemas

Question: every tool argument schema comes from the shared types, so each tool that takes mutations would spell out the whole vocabulary, and with output schemas the list an agent loads on connecting would run to megabytes.

Call: the tools that take a patch spell out the mutations they take; any other tool that takes a mutation describes it as one `apply_patch` defines. Output schemas are generated for every tool and its tests hold every output to them, but they are not listed over MCP. Parsing is as strict either way, and a refusal names the path that failed.

Alternatives: full schemas everywhere (megabytes in every agent's context); loose objects everywhere (agents guess shapes); schemas as MCP resources fetched on demand (clients differ in whether they read resources).

What would change it: MCP clients that keep tool schemas out of the model's context, or that need to validate structured output.
