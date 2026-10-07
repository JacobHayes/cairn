# Proof for brief 4.1: Service layer, composition root, capabilities, notifier

A host can now hand a patch to one service and get back what the PRD promises, over either
store: the patch is applied, what it newly caused is reported, and every subscriber hears
the revisions it moved.

## Before and after

The vendor evaluation run through the service, with a subscriber watching everything. The
memory store and a Turso file answered every step the same way.

| Step | Outcome | Journey revision | Subscriber heard |
|---|---|---|---|
| Create the journey | applied | 1 | journey at 1, deployment at 1 |
| Answer the up-front decisions | applied | 2 | journey at 2 |
| A note drafted at revision 1 | stale, nothing it touches overlaps | 2 | nothing |
| The client retries at revision 2 | applied | 3 | journey at 3 |
| The retry sent again after a lost response | answered from its receipt | 3 | nothing |
| Reach kickoff, complete environment access | applied | 4 | journey at 4 |
| An agent adds a dependency under finished work | applied; environment access now stale | 5 | journey at 5 |

Read back through the service, the journey equals the engine's own run of the same patches.
A subscriber arriving afterwards is handed the current revisions at once.
