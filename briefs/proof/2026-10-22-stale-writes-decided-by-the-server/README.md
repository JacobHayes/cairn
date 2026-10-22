# Proof: the server decides a stale write

A write drafted against an older revision is now decided once, by the server, for every caller:
a person in the browser, an agent over MCP, a script over REST. If nothing that changed since
is anything the write changes, it is applied at the current revision and the answer says what it
moved past. If something overlaps, it is refused as `stale`, as before. Before this, only the
web page and the Rust client resubmitted such a write (each with a loop of its own); an agent
or script was told `stale` and had to redo it.

Real answers on the vendor-evaluation fixture, two writers (a person and an agent):

| Situation | Before | After |
|---|---|---|
| The agent edits `n_access`, drafted at revision 2; meanwhile a person annotated `n_plan` (revision 3) | `stale`, moved past 2 to 3; MCP and REST callers redo the write | `applied` at revision 4, `over`: moved past 2 to 3, the person's note named as what intervened |
| Both, drafted at revision 4, rename the same node `n_workload`; the person's lands first | `stale` | `stale`, moved past 4 to 5; the person's title is kept (unchanged, and still refused) |
| A role removal drafted while the role was empty; meanwhile that same role is filled | `stale` (clients declined to retry) | `stale`; the fill is kept |
| The same, but the fill went to a different role | `stale` (only a client resubmitted it) | `applied` at revision 3, `over`: moved past 1 to 2; the other fill is kept |
| The agent's answer to the first row is lost and the same patch is resent unchanged | stale again | `already_applied` at revision 4, the same receipt |

The answer to a stale write that was applied carries `over`; in the REST answer it is next to
`receipt`, and the same field is in the MCP write tool's output. The served guide now tells an
agent to read the journey again after an answer that says `over` and after `already_applied`
(a lost answer may have hidden an `over`), because the receipt's revision then includes changes
it has not seen.

On the web nothing changes on screen: a disjoint concurrent edit landed silently before and
still does, now with no resubmission by the page.

## Known limits

- What the write read is not compared, only what it writes: a decision answered on a view where
  another node's note has changed since still lands. Guards run at apply, so no invariant breaks.
- A write that loses a race at commit is decided again, at most 32 times per request.
- A proposal's apply or edit and a route import are never applied past a change; they answer
  `stale` so a person reviews again.
