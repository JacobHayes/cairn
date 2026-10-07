# Proof for brief 4.3: MCP server and shipped instructions

An agent can now work a journey through Cairn's remote MCP endpoint (`/mcp`, behind the API's
auth layer) with a curated tool set; when it connects it receives the guide
(`instructions/SKILL.md`) as its instructions and each workflow in `instructions/workflows/`
as a prompt.

## An agent session over the vendor evaluation

The agent reads the snapshot first, lists the decisions it can answer now, and answers them
in rank order, each write naming its base revision and its own patch id:

| Decision | Answer | Journey revision after |
|---|---|---|
| `n_partner_runs` | no | 2 |
| `n_meeting_date` | 2026-11-20 | 3 |
| `n_purpose` | purchase | 4 |
| `n_who_informed` | both stakeholders | 5 |
| `n_who_owns` | the evaluation's lead | 6 |

Then:

- A patch with two mistakes (completing the workload placeholder before it is broken down,
  and a text answer to a yes-or-no decision) is refused with both violations, and nothing
  is written.
- The agent proposes breaking the placeholder into "Ingest workload" and "Query workload"
  under its own proposal id, reviews it (no violations), and applies it; the snapshot no
  longer lists anything needing a breakdown.
- Its acting frontier now opens with `n_kickoff` and `n_decision_meeting`, the same list
  the next view derives, and no decision it could answer is left.

## Known limits

- The endpoint is stateless and answers in JSON; it opens no server-sent event stream.
