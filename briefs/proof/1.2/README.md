# Proof for brief 1.2: Schema and fixtures

Cairn's documents (route files, journeys, patches, graphs) now parse and write
deterministically: every fixture is already in canonical form, and a document that breaks a
rule fails to parse with the path to the value that is wrong. `cairn-document <type> <file>`
checks a file and prints its canonical YAML or the error.

## What the parser does

- Every fixture route file and journey (vendor evaluation, product launch, hiring loop, bake-off)
  parses and writes back byte for byte.
- An estimate on a milestone is refused: `at nodes[6]: field estimate is not allowed on a
  milestone node (A1a)`, with its line and column.
- `fills_role` on a date decision is refused the same way, at its node.
- In the graph form a path or a role key where a node key belongs is refused: `"setup/access" is
  not a key of this type`.
- Limits hold at their value and refuse one past it:

| Limit | At the limit | One past |
|---|---|---|
| `node_count_max` (2,000) | 2,000 nodes parse | 2,001 refused at `nodes` |
| `choice_count_per_decision_max` (32) | 32 choices parse | 33 refused at `nodes[0].choices` |
| `offset_days_max` (365) | an estimate of 365 days parses | 366 refused at `nodes[8].estimate` |

- The generated `schema/route-file.schema.json` accepts every fixture route file and rejects what
  the parser rejects for a kind restriction or an unknown field.
