# Proof for brief 4.2: HTTP API, OpenAPI, SSE, TypeScript client

Everything the service offers is now an HTTP endpoint behind the auth layer, described by a
generated OpenAPI document (`openapi/cairn.json`) from which the TypeScript client's types
are generated, with a Rust client that retries a conflict only when that is safe.

## What it does

Observed with Ann and Bob signed in as two members of the team:

- A patch lands once. A note on the vendor evaluation lands at revision 2; sent again after a lost
  response it is answered from its receipt, and its patch id reused with other content is
  refused as a conflict. Nothing is applied twice.
- A patch with three independent mistakes (an illegal transition, a node that does not
  exist, a text answer to a yes-or-no decision) is refused with all three, each by path,
  and nothing is written.
- Ann and Bob note a small journey from the same revision. Bob's lands first; Ann's is stale,
  but what intervened cannot overlap a note, so her client resubmits it on its own and it
  lands at revision 3.
- Ann removes a group Bob has just renamed an action inside. What intervened overlaps the
  removal, so her client surfaces the conflict instead of retrying, and nothing is removed.
- A live stream watching the journey, the deployment, and a journey that does not exist
  starts from their current revisions:

  | Watched | First tick | After Bob's note |
  |---|---|---|
  | `j_vendor_eval` | revision 2 | revision 3 |
  | the deployment | revision 1 | - |
  | `j_none` (missing) | revision 0 | - |

- The domain document carries the stored journey and the caller's derive inputs, nothing
  derived: read at 2026-10-06T03:00Z it says today is 2026-10-05 in the deployment's zone
  (five hours behind UTC), and deriving it in the browser gives what the server derives.

## Known limits

- The Rust client speaks plain HTTP to a known socket address; TLS and name resolution wait
  for a client that leaves the machine.
