# Lists in tooling and documents derive from one owner

Question (from the rule-tooling unit): several scripts, a test and ARCHITECTURE each kept their own copy of a list that has an owner elsewhere, and the copies had drifted: the web build's staleness check missed the generated folders, the MCP tool-coverage test lacked a third of the PRD's tool surface, and ARCHITECTURE's layout and crate graph had fallen behind the workspace.

The call:

- The generated paths are owned by ARCHITECTURE (Generated artifacts). `scripts/generated-paths` reads them from the document; the generated step and the web build's staleness check in `scripts/test` take them from it. The generators still name their own output, since each writes only its path.
- The tool surface is owned by the PRD's tool-surface requirement. The coverage test splits that line into clauses and fails unless its table has exactly those clauses, each covered by tools that exist.
- The crate graph and layout are owned by the workspace: a test compares the graph with the manifests' dependencies (direct ones the others do not imply) and the layout with the crate directories. The document keeps the picture; the test keeps it true.
- Which browser specs are server-side is owned by the `@server` tag; `scripts/changed` and `scripts/test` already read the tag, so nothing changed there.

Alternatives: generate the graph and the layout into ARCHITECTURE (a generated file inside a hand-written document breaks the rule that generated files live only in generated paths); keep the copies and add reminders (they drift, as they did).

What would change it: a generator that could own the diagram without mixing generated text into the document, or a second place that needs the generated paths in a form the document cannot give.
