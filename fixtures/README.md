# Fixtures

Generic seed routes, each with a journey scenario, plus one journey with no route. The
engine's scenario tests, the in-browser host, and the server's integration tests all load
them (ARCHITECTURE, Testing the engine). Nothing here refers to a real use case, person, or
organization (PRD, Domain-free).

Each directory holds a `route.yaml` (a route file: the file format, which
`schema/route-file.schema.json` describes) and a `journey.yaml` (a scenario: patches in
order, each with the clock it applies at and who submits it). Each scenario runs in a fresh
deployment at revision 0. A patch that creates entities advances the deployment revision by
one; a later patch that refers to existing entities names that revision (E6). A scenario's
first patch
creates its journey at base revision 0 from version 1 of the route beside it; the route
file is that version. Route files carry every key, so scenario patches can refer to nodes,
roles, and kinds by key. Every file is written in canonical form: parsing and writing it
gives the same bytes, which `crates/schema/tests/fixtures.rs` checks.

## `vendor-evaluation/`

The PRD's illustrative example, with every mechanism it names. Four up-front decisions
(purchase or research, who owns the evaluation, who stays informed, and the decision-meeting
date) fill the `eval_owner` and `stakeholders` roles and pin the decision meeting. Setup is a
stage opened by the kickoff milestone; the test plan requires environment access and has two
child actions; the test workload is a placeholder. Testing holds a gated decision (the
comparison set requires the plan) whose answer decides whether the baseline is relevant, and a
partner-led subset relevant only when a partner runs the testing. Reporting holds the findings,
a late-filled `findings_reviewer`, and a final-review stage that opens 14 days before the
decision meeting and closes at it, with the final report (weight 5, artifact required) inside.
The decision meeting is the final milestone, weight 10. Resources: an access-request message
draft, a report template, and two example test plans. The scenario answers the up-front
decisions, reaches kickoff, breaks down the workload, answers the comparison set once the plan
is done, snoozes the baseline on a node, pins the report earlier than its derived due date,
finishes the workload and the baseline (which completes testing and opens reporting), and
answers the reviewer with a person created in the same patch.

Derived at each decision point (brief 2.2): the relevance and participations `derive` gives
after steps 1, 2, 6, and 8. The scenario matrix (`crates/engine/tests/matrix.rs`) checks these
values, and `briefs/proof/2.2/prove.sh` checks these tables against the engine's output.

| Node | created (step 1) | up-front decisions (step 2) | comparison set (step 6) | findings reviewer (step 8) |
|---|---|---|---|---|
| `testing/baseline` | undecided, by its condition | undecided, by its condition | relevant, by its condition | relevant, by its condition |
| `testing/partner-led` | undecided, by its condition | not relevant, by its condition | not relevant, by its condition | not relevant, by its condition |
| `testing/partner-led/criteria` | undecided, by `testing/partner-led` | not relevant, by `testing/partner-led` | not relevant, by `testing/partner-led` | not relevant, by `testing/partner-led` |
| `testing/partner-led/partner-results` | undecided, by `testing/partner-led` | not relevant, by `testing/partner-led` | not relevant, by `testing/partner-led` | not relevant, by `testing/partner-led` |

Every other node (23 at step 8) is relevant at every point, with no condition applying.

| Node and kind | created (step 1) | up-front decisions (step 2) | comparison set (step 6) | findings reviewer (step 8) |
|---|---|---|---|---|
| every node, `owner` | none (default owner `eval_owner`); all unassigned | `e_lead` (default owner `eval_owner`) | `e_lead` (default owner `eval_owner`) | `e_lead` (default owner `eval_owner`) |
| `reporting`, `informed` | none (role `stakeholders`) | `e_stakeholder_a`, `e_stakeholder_b` (role `stakeholders`) | `e_stakeholder_a`, `e_stakeholder_b` (role `stakeholders`) | `e_stakeholder_a`, `e_stakeholder_b` (role `stakeholders`) |
| `reporting/final-review`, `informed` | none (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) |
| `reporting/final-review/final-report`, `informed` | none (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) |
| `reporting/findings`, `informed` | none (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) |
| `reporting/findings-reviewer`, `informed` | none (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) |
| `reporting/review-opens`, `informed` | none (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) | `e_stakeholder_a`, `e_stakeholder_b` (from `reporting`) |
| `reporting/final-review/final-report`, `reviewer` | none (role `findings_reviewer`) | none (role `findings_reviewer`) | none (role `findings_reviewer`) | `e_reviewer` (role `findings_reviewer`) |

Due dates once the decision meeting is pinned (brief 2.3): answering the meeting date in step 2
pins `decision-meeting` to 2026-11-20 (E3), and every unfinished node a bound reaches gets
these latest starts and due dates. The final review closes at the meeting and opens 14 days
before it; everything upstream is due by what it feeds. Latest bounds read backward from pins
and actuals, so they do not depend on the day they are read. The scenario matrix checks these
values, and `briefs/proof/2.3/prove.sh` checks this table against the engine's output.

| Node | latest start | due |
|---|---|---|
| `setup/access` | 2026-10-29 | 2026-10-31 |
| `testing/baseline` | 2026-11-03 | 2026-11-06 |
| `testing/comparison-set` | 2026-11-03 | 2026-11-03 |
| `decision-meeting` | 2026-11-20 | 2026-11-20 |
| `reporting/final-review/final-report` | 2026-11-17 | 2026-11-20 |
| `reporting/final-review` | 2026-11-20 | 2026-11-20 |
| `reporting/findings` | 2026-11-15 | 2026-11-17 |
| `kickoff` | 2026-10-29 | 2026-10-29 |
| `setup/plan` | 2026-11-03 | 2026-11-03 |
| `setup/plan/draft` | 2026-10-31 | 2026-11-02 |
| `setup/plan/review` | 2026-11-02 | 2026-11-03 |
| `reporting/review-opens` | 2026-11-06 | 2026-11-06 |
| `testing` | 2026-11-06 | 2026-11-06 |

## `hiring-loop/`

A small route with deep containment (interview loop, onsite, debrief, notes, scorecard) and a
multi-valued `panel` role filled by an entity-list decision. The hiring manager role has no
filling decision and is filled directly. The interviews deliverable is a placeholder with an
`interviewer` participation on the panel role. An offer decision gates two branches by
condition: the offer letter when it is yes, the close-out otherwise. The scenario fills the
panel, skips the screen with a reason, has the assistant break the interviews into one
interview per panelist with explicit participations, completes the onsite, and makes the offer.

## `product-launch/`

Many milestones and date rules and no conditions. The launch is the final milestone; code
freeze is due 21 days before it, the beta starts no sooner than two days after the freeze (and
reaches itself on its date) and ends ten days after it starts, the documentation is due a week
before launch, the announcement three days before and no sooner than the beta's end, the
go/no-go a day before, and the retrospective no sooner than five days after. Build and beta are
stages bounded by milestones on both ends. The launch materials name the multi-valued `writers`
role through a multi-valued `contributor` kind (owner, being single-valued, cannot take a
multi-valued role, A7). The scenario pins the launch date, reaches kickoff,
shifts the launch pin a week when feature work slips, and records a code-freeze actual
date later than the plan's chain allows: a shortfall, which is reported, never rejected.
Derived (brief 2.3): with the launch pinned to 2026-11-23, the freeze is due 21 days before it,
on 2026-11-02; reached on 2026-11-04, it is short by 2 days, and so is the launch.

## `bake-off/`

The PRD's ad-hoc sibling: a journey with no route. It is created empty; the assistant drafts a
two-week bake-off between two options as a proposal (three decisions, four deliverables, one
milestone, two entities, and a `judges` role declared on the journey); the owner applies it; the
criteria and judges are decided and both trials start.
