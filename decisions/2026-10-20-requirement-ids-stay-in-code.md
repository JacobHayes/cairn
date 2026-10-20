# Requirement ids stay in code, never in text people or agents read

The owner objected to rule codes (a requirement id such as A2 or "(E3, F5)") reaching
people at all, including being quoted by an agent. Violation and error messages carried
them to API and MCP clients verbatim, and the web app stripped one trailing id with a
regex. Tool, OpenAPI and route-file schema descriptions were generated from doc comments
that carried ids as well.

## The call

- Every message, description and served guide says the rule in words and carries no id.
  The web app's stripping is deleted; nothing sends ids, so there is no second path.
- Ids stay in `//` and `///` comments that never reach a served description, in test
  names, and in `assert!` messages.
- A `///` comment is a served description when its item derives a schema (schemars,
  OpenAPI, MCP). On those the doc text is free of ids and the citation moves to a plain
  `// PRD: A2.` comment directly above the doc comment, which no generator reads.
- The same holds for the names of the repository's documents ("PRD glossary, Pin",
  "ARCHITECTURE, File format"): a reader of the API or the schemas cannot open them. The
  description says the thing itself and a `// PRD: ...` or `// ARCHITECTURE: ...` comment
  above it keeps the pointer.
- A guard (`served_text` in the integration tests) fails on an id or a document name in the OpenAPI document,
  the route-file schema, any MCP tool's description or schemas, the guide and workflows,
  and in the message of any violation an engine test's `apply` produces. It detects the
  shape of an id and pins no wording.

## Alternatives

- Keep ids and strip them at each edge: leaves a second path in every client and an
  agent still reads them over MCP.
- Carry the id as a separate field on a violation for developers: no reader needs it,
  and the violation code already names the rule for programs.

## What would change it

A client that needs to link a violation to the PRD would justify a structured field
(not text in the message).
