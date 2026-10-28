# Proof: one tool makes every proposal

An agent connected over MCP now starts every proposal with `create_proposal`: its own draft
with a destination, or one Cairn drafts, an upgrade, a save as route or a re-link. The three
tools that drafted those are gone, and the tool's schema says it takes exactly one content.

## The tool list

Measured on the demo's `tools/list`, in o200k tokens of each tool's compact JSON.

| | Before | After |
|---|---:|---:|
| Tools | 23 | 20 |
| Listed tokens | 10,051 | 9,839 |
| Tokens for proposal-making tools | 1,190 (four tools) | 978 (one tool) |

## Observed on the demo

| Asking for | Before | After |
|---|---|---|
| A save as route of the bake-off journey | one call to `save_as_route` | one call to `create_proposal` naming `save_as_route`: an open proposal at revision 1, destined for the new route |
| A save as route and a re-link in one call | not expressible | refused at `relink`: a proposal holds exactly one content |

An upgrade and a re-link work the same way, each still one call.

## Known limits

- A client that does not read `oneOf` in a schema learns of a mixed call only from the
  refusal.
