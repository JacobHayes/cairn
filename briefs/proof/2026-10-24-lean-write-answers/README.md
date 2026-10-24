# Proof: lean write answers and errors

An agent that writes through the tools now gets back its new `revision` and what the write caused,
and nothing it sent. A refusal is one block of JSON whose `message` says how to recover. Measured
against the demo fixtures plus a 214-node journey, in o200k tokens of compact JSON, as an agent
client shows them (one copy of a success, every text block of an error).

| Answer | Before | After |
|---|---:|---:|
| Write (`transition_node start`) | 73 | 10 |
| Write with consequences (`answer_decision`) | 126 | 56 |
| `create_proposal`, a one-node draft (grows with the draft before, constant after) | 187 | 33 |
| Refusal, a guard failure (path and code kept) | 110 | 108 |
| Refusal, stale, on the 214-node journey | 5,231 | 1,402 |

## What an agent reads

- A write: `{"revision":6,"status":"applied"}`. No content hash, domain or patch id; `over`,
  `consequences` and `notices` appear only when present.
- A proposal write: its id, destination, editing revision, destination revision and state. The
  draft is read with `get_proposal`.
- A stale refusal: `message` names the journey in words and its current revision ("journey j_big
  is at revision 3, not 0; read it again ... write again with a new patch_id"). `conflicts` keep the
  numbers; `intervening` and `touched` are grouped by graph, then record kind, so the same key in
  two graphs stays distinct.
- An invalid refusal keeps every violation with its path. Messages no longer print Rust debug
  forms ("fails HasArtifact" is now "fails the has_artifact guard").
- The in-app assistant reads the new answers: its report, its ten-node rule and its proposal
  destinations work as before; its applied action carries the patch id, domain and revision.

## Known limits

- The stale refusal on a journey that changed everywhere is still a list of every changed node key.
