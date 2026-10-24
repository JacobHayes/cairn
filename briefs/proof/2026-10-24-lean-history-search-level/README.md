# Lean history, search and level answers

An agent reading a journey's history now gets each patch once, with its events as a type
and a subject, instead of every stored change; a search answer groups where the text was
found; a level's edges are just `from`, `to` and `implicit`. Ask for the stored changes with
`get_history`'s `deltas: true`.

## Size, before and after

Real answers from `cairn demo`, over MCP, as compact JSON; tokens counted with tiktoken
`o200k_base` (a stated proxy). The large journey is a generated 214-node journey.

| Read | Tokens before | Tokens after | Bytes before | Bytes after |
|---|---:|---:|---:|---:|
| history, bake-off | 3,008 | 327 (-89%) | 10,396 | 1,082 |
| history, hiring loop | 2,931 | 569 (-81%) | 9,928 | 1,992 |
| history, product launch | 2,189 | 412 (-81%) | 7,594 | 1,452 |
| history, vendor evaluation | 5,361 | 880 (-84%) | 19,094 | 3,129 |
| history, large journey | 9,180 | 78 (-99%) | 31,156 | 196 |
| history, vendor evaluation, with `deltas` | 5,361 | 3,798 (-29%) | 19,094 | 13,620 |
| level, bake-off | 341 | 206 (-40%) | 1,293 | 761 |
| level, vendor evaluation | 1,244 | 987 (-21%) | 4,393 | 3,364 |
| level, large journey | 9,164 | 6,122 (-33%) | 28,044 | 18,335 |
| search "a" | 3,125 | 1,648 (-47%) | 9,764 | 3,807 |
| search "the" | 361 | 252 (-30%) | 1,358 | 904 |

## What an answer carries now

- History: per patch its id, actor, time and note, and its events as `type` and `subject`
  (an event names its own actor when it differs from the patch's, and the confirming user
  when a proposal was applied). The same patches in the same order with or without `deltas`.
- Search: per journey, the hits grouped by where they were found (journey name or
  description, node titles and descriptions as keys, notes by key); a resource hit keeps
  its node and its own key.
- Level: an edge is `from`, `to` and `implicit`, no longer the list of edges it stands for.
- The HTTP history, search and level reads, and the app that renders from them, keep their
  full shapes.

## Known limits

- A level's nodes are most of its size; a page of them is unchanged.
- Tokens are a proxy (tiktoken), not Anthropic's tokenizer.
